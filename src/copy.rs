use std::sync::{Arc, Mutex};

use std::error::Error;
use std::io::Write;
use std::os::unix::io::AsRawFd;

use byteorder::{NativeEndian, WriteBytesExt};

use wayland_client::protocol::{
    wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer, wl_seat, wl_shell, wl_shm,
};

use wayland_client::{Display, EventQueue, Filter, GlobalManager};

event_enum!(
    Events |
    Device => wl_data_device::WlDataDevice,
    Offer => wl_data_offer::WlDataOffer
);

fn start_surface(globals: &GlobalManager) -> Result<(), String> {
    // buffer (and window) width and height
    let buf_x: u32 = 1;
    let buf_y: u32 = 1;

    // create a tempfile to write the contents of the window on
    let mut tmp = tempfile::tempfile().expect("Unable to create a tempfile.");
    // write the contents to it, lets put a nice color gradient
    let _ = tmp.write_u32::<NativeEndian>(0);
    let _ = tmp.flush();

    /*
     * Init wayland objects
     */

    // The compositor allows us to creates surfaces
    let surface = match globals.instantiate_exact::<wl_compositor::WlCompositor>(1) {
        Ok(compositor) => compositor.create_surface(),
        Err(e) => return Err(e.to_string()),
    };

    // The SHM allows us to share memory with the server, and create buffers
    // on this shared memory to paint our surfaces
    let pool = match globals.instantiate_exact::<wl_shm::WlShm>(1) {
        Ok(shm) => shm.create_pool(
            tmp.as_raw_fd(),            // RawFd to the tempfile serving as shared memory
            (buf_x * buf_y * 4) as i32, // size in bytes of the shared memory (4 bytes per pixel)
        ),
        Err(e) => return Err(format!("Shm: {}", e.to_string())),
    };

    let buffer = pool.create_buffer(
        0,                        // Start of the buffer in the pool
        buf_x as i32,             // width of the buffer in pixels
        buf_y as i32,             // height of the buffer in pixels
        (buf_x * 4) as i32,       // number of bytes between the beginning of two consecutive lines
        wl_shm::Format::Argb8888, // chosen encoding for the data
    );

    // The shell allows us to define our surface as a "toplevel", meaning the
    // server will treat it as a window
    //
    // NOTE: the wl_shell interface is actually deprecated in favour of the xdg_shell
    // protocol, available in wayland-protocols. But this will do for this example.
    let shell_surface = match globals.instantiate_exact::<wl_shell::WlShell>(1) {
        Ok(shell) => shell.get_shell_surface(&surface),
        Err(e) => return Err(format!("Shell: {}", e.to_string())),
    };

    shell_surface.assign_mono(|shell_surface, event| {
        use wayland_client::protocol::wl_shell_surface::Event;
        // This ping/pong mechanism is used by the wayland server to detect
        // unresponsive applications
        if let Event::Ping { serial } = event {
            shell_surface.pong(serial);
        }
    });

    // Set our surface as toplevel and define its contents
    shell_surface.set_toplevel();
    surface.attach(Some(&buffer), 0, 0);
    surface.commit();

    Ok(())
}

fn get_offers(
    globals: &GlobalManager,
    event_queue: &mut EventQueue,
) -> Result<
    Arc<Mutex<std::collections::HashMap<String, wayland_client::Main<wl_data_offer::WlDataOffer>>>>,
    String,
> {
    let offers = Arc::new(Mutex::new(std::collections::HashMap::new()));
    let cloned_offers = offers.clone();

    let filter = Filter::new(move |e, filter| match e {
        Events::Offer { event, object } => match event {
            wl_data_offer::Event::Offer { mime_type } => {
                debug!("Got a mime!");
                cloned_offers.lock().unwrap().insert(mime_type, object);
            }
            _ => (),
        },
        Events::Device { event, .. } => match event {
            wl_data_device::Event::DataOffer { id } => {
                debug!("Got an offer!");
                id.assign(filter.clone());
            }
            _ => {}
        },
    });

    let seat = match globals.instantiate_range::<wl_seat::WlSeat>(1, 10) {
        Ok(s) => s,
        Err(e) => return Err(format!("Seat: {}", e.to_string())),
    };

    let data_device =
        match globals.instantiate_range::<wl_data_device_manager::WlDataDeviceManager>(1, 10) {
            Ok(dm) => dm.get_data_device(&seat),
            Err(e) => return Err(format!("Device manager: {}", e.description())),
        };

    data_device.assign(filter.clone());

    debug!("Number of offers: {}", offers.lock().unwrap().len());
    debug!("Hoping to fetch the DataOffer");
    if let Err(e) = event_queue.dispatch(|_, _| ()) {
        return Err(e.to_string());
    }

    debug!("Number of offers: {}", offers.lock().unwrap().len());
    debug!("Registered filters, hoping to fetch the Mime types");
    if let Err(e) = event_queue.dispatch(|_, _| ()) {
        return Err(e.to_string());
    }
    debug!("Number of offers: {}", offers.lock().unwrap().len());

    Ok(offers.clone())
}

pub fn load_clipboard_content(show_mime: bool) -> Result<String, String> {
    debug!("Show mime: {}", show_mime);
    // Connect to the server

    let display = match Display::connect_to_env() {
        Ok(d) => d,
        Err(e) => return Err(format!("Cannot connect to wayland: {}", e.to_string())),
    };

    let mut event_queue = display.create_event_queue();

    let attached_display = (*display).clone().attach(event_queue.get_token());
    let globals = GlobalManager::new(&attached_display);

    if let Err(e) = event_queue.sync_roundtrip(|_, _| ()) {
        return Err(e.to_string());
    }

    if let Err(e) = start_surface(&globals) {
        return Err(e.to_string());
    }

    debug!("Getting offers:");
    let offers = match get_offers(&globals, &mut event_queue) {
        Ok(offers) => match event_queue.sync_roundtrip(|_, _| unreachable!()) {
            Ok(_) => offers,
            Err(e) => return Err(e.to_string()),
        },
        Err(e) => return Err(e),
    };

    for (mime, offer) in offers.lock().unwrap().clone() {
        debug!("mime: {}", mime);
        if mime == "text/plain;charset=utf-8" {
            let pipes = nix::unistd::pipe().unwrap();
            let reader_pipe = pipes.0;
            let writer_pipe = pipes.1;

            debug!("receive()");
            offer.receive(mime, writer_pipe);
            if let Err(e) = event_queue.sync_roundtrip(|_, _| ()) {
                return Err(e.to_string());
            }
            nix::unistd::close(writer_pipe).expect("Error closing writer descriptor");

            let mut buf = [0; 4096];
            let mut result = Vec::<u8>::new();

            loop {
                debug!("Reading chunck");
                match nix::unistd::read(reader_pipe, &mut buf) {
                    Ok(size) => {
                        debug!("Processing chunck");
                        if size > 0 {
                            result.extend(buf.iter());
                        } else {
                            break;
                        }
                    }
                    Err(error) => {
                        return Err(format!("Error when reading: {}", error));
                    }
                }
            }
            debug!("All chuncks read");
            return String::from_utf8(result).or_else(|z| Err(z.to_string()));
        }
    }
    Err("No suitable data found".to_string())
}
