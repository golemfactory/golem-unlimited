use smallvec::{SmallVec, Array};
use cl_sys::*;
use std::ptr;

#[derive(Debug)]
pub struct Error(cl_int);

type PlatformsInner = SmallVec<[cl_platform_id; 2]>;
pub struct Platforms(<PlatformsInner as IntoIterator>::IntoIter);

impl Platforms {
    pub fn try_new() -> Result<Self, Error> {
        let mut num_platforms: cl_uint = 0;
        let ret = unsafe { clGetPlatformIDs(0, ptr::null_mut(), &mut num_platforms as *mut cl_uint) };
        if ret != CL_SUCCESS {
            return Err(Error(ret))
        }
        let mut platforms = SmallVec::with_capacity(num_platforms as usize);
        unsafe { platforms.set_len(num_platforms as usize)}
        let ret = unsafe { clGetPlatformIDs(platforms.capacity() as u32, platforms.as_mut().as_mut_ptr(), &mut num_platforms as *mut cl_uint) };
        if ret != CL_SUCCESS {
            return Err(Error(ret))
        }
        Ok(Platforms(platforms.into_iter()))
    }
}

impl Iterator for Platforms {
    type Item = Platform;

    #[inline]
    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        match self.0.next() {
            Some(v) => Some(Platform(v)),
            None => None
        }
    }
}

pub struct Platform(cl_platform_id);

#[inline(always)]
unsafe fn extract_string<F : Fn(usize, *mut c_void, *mut usize) -> cl_int>(f : F) -> Result<String, Error> {
    let mut size = 0;
    let ret = f(0, ptr::null_mut(), &mut size as *mut usize);
    if ret != CL_SUCCESS {
        return Err(Error(ret))
    }
    let mut v : Vec<u8> = Vec::with_capacity(size);
    v.set_len(size-1);
    let ret = f(v.capacity(), v.as_mut_ptr() as *mut c_void, ptr::null_mut());
    if ret != CL_SUCCESS {
        return Err(Error(ret))
    }
    Ok(String::from_utf8(v).unwrap())
}

#[inline(always)]
unsafe fn extract_vec<A, F>(f : F) -> Result<SmallVec<A>, Error>
    where F :  Fn(cl_uint, *mut A::Item, *mut cl_uint) -> cl_int, A: Array {
    let mut num_items : cl_uint = 0;
    let ret = f(0, ptr::null_mut(), &mut num_items as *mut cl_uint);
    if ret != CL_SUCCESS {
        return Err(Error(ret))
    }
    let mut items = SmallVec::with_capacity(num_items as usize);
    items.set_len(num_items as usize);
    let ret = f(num_items, items.as_mut().as_mut_ptr(), ptr::null_mut());
    if ret != CL_SUCCESS {
        return Err(Error(ret))
    }
    Ok(items)
}

impl Platform {

    pub fn devices(&self) -> Result<Devices, Error> {
        let devices = unsafe { extract_vec(|num_entries, devices, num_devices|
            clGetDeviceIDs(self.0, CL_DEVICE_TYPE_GPU|CL_DEVICE_TYPE_ACCELERATOR, num_entries, devices, num_devices))? };
        Ok(Devices(devices.into_iter()))
    }

    pub fn name(&self) -> String {
        unsafe {
            extract_string(|param_value_size, param_value, param_value_size_ret|
                clGetPlatformInfo(self.0, CL_PLATFORM_NAME, param_value_size, param_value, param_value_size_ret))
                .unwrap()
        }
    }
}

type DevicesInner = SmallVec<[cl_device_id; 5]>;

pub struct Devices(<DevicesInner as IntoIterator>::IntoIter);

impl Iterator for Devices {
    type Item = Device;

    #[inline]
    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        match self.0.next() {
            Some(device_id) => Some(Device(device_id)),
            None => None
        }
    }
}


pub struct Device(cl_device_id);

impl Device {

    #[inline]
    unsafe fn extract(&self, param_name : cl_uint) -> Result<String, Error> {
        extract_string(|param_value_size, param_value, param_value_size_ret|
            clGetDeviceInfo(self.0, param_name, param_value_size, param_value, param_value_size_ret))
    }

    #[inline]
    pub fn name(&self) -> String {
        unsafe { self.extract(CL_DEVICE_NAME).unwrap() }
    }

    #[inline]
    pub fn vendor(&self) -> String {
        unsafe { self.extract(CL_DEVICE_VENDOR).unwrap() }
    }


}