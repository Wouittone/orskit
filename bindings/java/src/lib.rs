//! Java FFM (Foreign Function & Memory) bindings for orskit
//!
//! This library provides C-compatible interfaces for use with Java's
//! Foreign Function & Memory API (available in Java 21+, stable in Java 25+).

/// Represents an orbital state for FFM interop
#[repr(C)]
pub struct FFMOrbitalState {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
}

/// Create a new orbital state from FFM struct
#[no_mangle]
pub extern "C" fn orbital_state_new(
    x: f64,
    y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    vz: f64,
) -> *mut FFMOrbitalState {
    Box::into_raw(Box::new(FFMOrbitalState { x, y, z, vx, vy, vz }))
}

/// Deallocate orbital state
#[no_mangle]
pub extern "C" fn orbital_state_free(ptr: *mut FFMOrbitalState) {
    if !ptr.is_null() {
        unsafe {
            let _ = Box::from_raw(ptr);
        }
    }
}

/// Get position vector
#[no_mangle]
pub extern "C" fn orbital_state_get_position(
    state: *const FFMOrbitalState,
    out: *mut [f64; 3],
) {
    if !state.is_null() && !out.is_null() {
        unsafe {
            let s = &*state;
            (*out)[0] = s.x;
            (*out)[1] = s.y;
            (*out)[2] = s.z;
        }
    }
}

/// Get velocity vector
#[no_mangle]
pub extern "C" fn orbital_state_get_velocity(
    state: *const FFMOrbitalState,
    out: *mut [f64; 3],
) {
    if !state.is_null() && !out.is_null() {
        unsafe {
            let s = &*state;
            (*out)[0] = s.vx;
            (*out)[1] = s.vy;
            (*out)[2] = s.vz;
        }
    }
}
