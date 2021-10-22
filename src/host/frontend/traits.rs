
use std::sync::{Arc, Mutex};

use crate::error::Error;

//pub trait Canvas: Send {
//}

//pub trait Audio {
//
//}

//pub type SharedCanvas = Arc<Mutex<Box<dyn Canvas>>>;
//pub type SharedAudio = Arc<Mutex<Box<dyn Audio>>>;

// TODO instead make something like HostAdapter, or a representation of the backend, which it's given to the builder function
pub trait Frontend {
    //fn set_size(&mut self, x: u32, y: u32);
    //fn draw_bitmap(&mut self, x: u32, y: u32, bitmap: &[u8]);
    //fn set_update_callback(&mut self, update: Box<fn(&mut [u8]) -> ()>);
    fn request_update(&self, x: u32, y: u32, bitmap: &[u8]);
}

struct HostAdapter {
    bitmap: Mutex<Vec<u32>>,
}



// Types:
//  Window (gfx out + input)
//  Audio
//  TTY
//  Network


/*

// Opt 1 - Simple Callback

pub trait Window {
    fn draw_bitmap(&self, x: u32, y: u32, bitmap: &[u32]);
}

pub trait Host {
    fn register_update<T, W: Window>(&mut self, func: fn(T, W), data: T);
}

// TODO how will the object data be shared with the device

*/


/*
// Opt 4 - The Host Generic Device Method

pub trait Window {
    fn request_update(&self, x: u32, y: u32, bitmap: &[u32]);
}

pub trait Host {
    fn create_window<W: Window>(&mut self) -> W;
}

pub struct YmDevice<W: Window> {
    pub window: W,
    pub buffer: Vec<u32>,
}

impl<W: Window> YmDevice<W> {
    pub fn new<H: Host>(host: &mut H) -> YmDevice<W> {
        YmDevice {
            window: host.create_window(),
            buffer: vec![0; 200 * 200],
        }
    }

    pub fn step(&mut self) {
        self.window.request_update(200, 200, self.buffer.as_slice());
    }
}

pub struct CustomWindow {
    pub buffer: Mutex<Vec<u32>>,
}

impl CustomWindow {
    fn request_update(&self, x: u32, y: u32, bitmap: &[u32]) {
        let mut target = self.buffer.lock().unwrap();
        for i in 0..target.len() {
            target[i] = bitmap[i];
        }
    }
}
*/



/*
// Opt 2 - The Callback Through Trait Method

pub trait Window {
    fn render(&self, x: u32, y: u32, bitmap: &mut [u8]);
}

pub trait Host {
    fn register_window<W: Window>(&mut self, window: Arc<W>) -> Result<(), Error>;
}



pub struct YmDevice(Arc<YmDeviceInternal>);

pub struct YmDeviceInternal {
    //pub window: Window,
    // some things
}

impl YmDevice {
    pub fn new<H: Host>(host: &mut H) -> YmDevice {
        let device = Arc::new(YmDeviceInternal {

        });

        host.register_window(device.clone()).unwrap();
        YmDevice(device)
    }
}

impl Window for YmDeviceInternal {
    fn render(&self, x: u32, y: u32, bitmap: &mut [u8]) {
        println!("here");
    }
}

impl Addressable for YmDevice {

}

*/


/*
// Opt 3 - The Callback Through Common Backend-defined Object Method

pub struct Window {
    width: u32,
    height: u32,
    buffer: Vec<u32>
}

impl Window {
    fn render(&self, x: u32, y: u32, bitmap: &mut [u8]) {
        
    }
}

pub trait Host {
    fn register_window<W: Window>(&mut self, window: Arc<W>) -> Result<(), Error>;
}



pub struct YmDevice(Arc<YmDeviceInternal>);

pub struct YmDeviceInternal {
    //pub window: Window,
    // some things
}

impl YmDevice {
    pub fn new<H: Host>(host: &mut H) -> YmDevice {
        let device = Arc::new(YmDeviceInternal {

        });

        host.register_window(device.clone()).unwrap();
        YmDevice(device)
    }
}

impl Window for YmDeviceInternal {
    fn render(&self, x: u32, y: u32, bitmap: &mut [u8]) {
        println!("here");
    }
}

//impl Addressable for YmDevice {
//
//}
*/

pub trait Host {
    fn add_window(&self, window: Box<dyn Window>);
    //fn create_pty(&self) -> Tty;
}

// TODO should you rename this Drawable, FrameUpdater, WindowUpdater?
pub trait Window: Send {
    fn update_frame(&mut self, width: u32, height: u32, bitmap: &mut [u32]);
}



#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u32>,
}

pub struct FrameSwapper {
    pub current: Frame,
    pub previous: Frame,
}

impl FrameSwapper {
    pub fn new() -> FrameSwapper {
        FrameSwapper {
            current: Frame { width: 0, height: 0, bitmap: vec![] },
            previous: Frame { width: 0, height: 0, bitmap: vec![] },
        }
    }
}

impl Window for FrameSwapper {
    fn update_frame(&mut self, width: u32, height: u32, bitmap: &mut [u32]) {
        std::mem::swap(&mut self.current, &mut self.previous);
        println!("{} {}", self.current.width, self.current.height);
        if self.current.width != width || self.current.height != height {
            self.current.width = width;
            self.current.height = height;
            self.current.bitmap.resize((width * height) as usize, 0);
            self.previous = self.current.clone();
            return;
        }

        for i in 0..(width as usize * height as usize) {
            bitmap[i] = self.current.bitmap[i];
        }
    }
}

pub struct FrameSwapperWrapper(Arc<Mutex<FrameSwapper>>);

impl Window for FrameSwapperWrapper {
    fn update_frame(&mut self, width: u32, height: u32, bitmap: &mut [u32]) {
        self.0.lock().map(|mut swapper| swapper.update_frame(width, height, bitmap));
    }
}


pub struct YmDeviceTransmutable(YmDevice);

pub struct YmDevice {
    pub count2: u32,
    pub count: Mutex<u32>,
    pub frame_swapper: Arc<Mutex<FrameSwapper>>,
}

impl YmDevice {
    pub fn new<H: Host>(host: &H) -> YmDeviceTransmutable {
        let frame_swapper = Arc::new(Mutex::new(FrameSwapper::new()));
        let device = YmDevice {
            count2: 0,
            count: Mutex::new(0),
            frame_swapper,
        };

        host.add_window(Box::new(FrameSwapperWrapper(device.frame_swapper.clone())));
        YmDeviceTransmutable(device)
    }
}

use crate::system::System;
use crate::devices::{Clock, Address, Transmutable, Steppable, Addressable, MAX_READ};
impl Transmutable for YmDeviceTransmutable {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}

impl Steppable for YmDeviceTransmutable {
    fn step(&mut self, system: &System) -> Result<Clock, Error> {    
        self.0.count2 += 1;
        if self.0.count2 > 1000 {
            self.0.count2 = 0;

            let value = match self.0.count.lock() {
                Ok(mut value) => { *value = *value + 1; *value }
                _ => { 0 },
            };

            let mut frame = self.0.frame_swapper.lock().unwrap();
            for i in 0..(frame.current.width * frame.current.height) {
                if i == value {
                    frame.current.bitmap[i as usize] = 0;
                } else {
                    frame.current.bitmap[i as usize] = 12465;
                }
            }
        }
        Ok(1)
    }
}

impl Addressable for YmDeviceTransmutable {
    fn len(&self) -> usize {
        0x20
    }

    fn read(&mut self, addr: Address, _count: usize) -> Result<[u8; MAX_READ], Error> {
        let mut data = [0; MAX_READ];

        debug!("read from register {:x}", addr);
        Ok(data)
    }

    fn write(&mut self, addr: Address, data: &[u8]) -> Result<(), Error> {
        debug!("write to register {:x} with {:x}", addr, data[0]);
        Ok(())
    }
}


/*
pub struct CustomWindow {
    pub buffer: Mutex<Vec<u32>>,
}

impl CustomWindow {
    fn request_update(&self, x: u32, y: u32, bitmap: &[u32]) {
        let mut target = self.buffer.lock().unwrap();
        for i in 0..target.len() {
            target[i] = bitmap[i];
        }
    }
}
*/


