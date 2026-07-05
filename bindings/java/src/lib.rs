//! Experimental C ABI for Java's Foreign Function & Memory API.

use std::ptr;

use orskit_core::{
    AttitudeState, CartesianState, FramedAngularVelocity, InertiaTensor, Orientation,
};
use orskit_frames::{CustomFrameId, FrameOrientation, FrameOrigin, ReferenceFrame};
use orskit_units::uom::si::{moment_of_inertia::kilogram_square_meter, ratio::ratio};
use orskit_units::{AngularVelocityVector, MomentOfInertia, Position, Ratio, VelocityVector};

/// Opaque handle owned by the foreign caller.
pub struct FFMSpacecraftState {
    state: CartesianState,
}

/// Creates a typed spacecraft state and returns an opaque owned handle.
///
/// Frame codes are: `0 = ICRF`, `1 = GCRF`, `2 = EME2000`, `3 = ITRF2020`,
/// and `4 = TEME`. Orientation uses scalar/x/y/z quaternion order from the
/// custom body frame into that reference frame. The three inertia inputs are
/// principal moments in `kg*m^2`, expressed in the custom body frame. Angular
/// velocity is expressed in body-frame radians per second. A null pointer
/// reports invalid input.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn spacecraft_state_new(
    x_m: f64,
    y_m: f64,
    z_m: f64,
    vx_m_s: f64,
    vy_m_s: f64,
    vz_m_s: f64,
    mass_kg: f64,
    epoch_tai_seconds: f64,
    orientation_w: f64,
    orientation_x: f64,
    orientation_y: f64,
    orientation_z: f64,
    angular_velocity_x_rad_s: f64,
    angular_velocity_y_rad_s: f64,
    angular_velocity_z_rad_s: f64,
    inertia_xx_kg_m2: f64,
    inertia_yy_kg_m2: f64,
    inertia_zz_kg_m2: f64,
    body_frame_id: u64,
    frame_code: u32,
) -> *mut FFMSpacecraftState {
    if !epoch_tai_seconds.is_finite() || !mass_kg.is_finite() || mass_kg <= 0.0 {
        return ptr::null_mut();
    }
    let Some(frame) = frame_from_code(frame_code) else {
        return ptr::null_mut();
    };
    let Ok(state) = CartesianState::new(
        frame,
        Position::from_metres(x_m, y_m, z_m),
        VelocityVector::from_metres_per_second(vx_m_s, vy_m_s, vz_m_s),
    ) else {
        return ptr::null_mut();
    };
    let body_id = CustomFrameId::new(body_frame_id);
    let body_frame = ReferenceFrame::new(
        FrameOrigin::Custom(body_id),
        FrameOrientation::Custom(body_id),
    );
    let Ok(orientation) = Orientation::from_quaternion(
        body_frame,
        frame,
        Ratio::new::<ratio>(orientation_w),
        Ratio::new::<ratio>(orientation_x),
        Ratio::new::<ratio>(orientation_y),
        Ratio::new::<ratio>(orientation_z),
    ) else {
        return ptr::null_mut();
    };
    let Ok(angular_velocity) = FramedAngularVelocity::new(
        AngularVelocityVector::from_radians_per_second(
            angular_velocity_x_rad_s,
            angular_velocity_y_rad_s,
            angular_velocity_z_rad_s,
        ),
        body_frame,
    ) else {
        return ptr::null_mut();
    };
    let Ok(attitude) = AttitudeState::new(orientation, angular_velocity) else {
        return ptr::null_mut();
    };
    let Ok(inertia) = InertiaTensor::principal(
        body_frame,
        MomentOfInertia::new::<kilogram_square_meter>(inertia_xx_kg_m2),
        MomentOfInertia::new::<kilogram_square_meter>(inertia_yy_kg_m2),
        MomentOfInertia::new::<kilogram_square_meter>(inertia_zz_kg_m2),
    ) else {
        return ptr::null_mut();
    };
    // The experimental binding does not yet expose the core spacecraft/view
    // split; construction still validates all supplied rigid-body values.
    let _validated_rigid_body = (inertia, attitude);

    Box::into_raw(Box::new(FFMSpacecraftState { state }))
}

/// Frees a handle returned by [`spacecraft_state_new`].
///
/// # Safety
///
/// `state` must be null or a live handle returned by `spacecraft_state_new`
/// that has not previously been freed.
#[no_mangle]
pub unsafe extern "C" fn spacecraft_state_free(state: *mut FFMSpacecraftState) {
    if !state.is_null() {
        // SAFETY: The caller contract guarantees unique ownership of a live handle.
        drop(unsafe { Box::from_raw(state) });
    }
}

/// Writes x/y/z position components in metres and returns whether it succeeded.
///
/// # Safety
///
/// `state` must point to a live handle and `out_xyz_m` must point to writable
/// storage for at least three consecutive `f64` values.
#[no_mangle]
pub unsafe extern "C" fn spacecraft_state_get_position_m(
    state: *const FFMSpacecraftState,
    out_xyz_m: *mut f64,
) -> bool {
    if state.is_null() || out_xyz_m.is_null() {
        return false;
    }
    // SAFETY: Pointer validity and output capacity are required by the caller contract.
    let values = unsafe { (*state).state.position().to_metres() };
    // SAFETY: The caller guarantees room for all three values and the source is local.
    unsafe { ptr::copy_nonoverlapping(values.as_ptr(), out_xyz_m, values.len()) };
    true
}

/// Writes x/y/z velocity components in metres per second.
///
/// # Safety
///
/// `state` must point to a live handle and `out_xyz_m_s` must point to writable
/// storage for at least three consecutive `f64` values.
#[no_mangle]
pub unsafe extern "C" fn spacecraft_state_get_velocity_m_s(
    state: *const FFMSpacecraftState,
    out_xyz_m_s: *mut f64,
) -> bool {
    if state.is_null() || out_xyz_m_s.is_null() {
        return false;
    }
    // SAFETY: Pointer validity and output capacity are required by the caller contract.
    let values = unsafe { (*state).state.velocity().to_metres_per_second() };
    // SAFETY: The caller guarantees room for all three values and the source is local.
    unsafe { ptr::copy_nonoverlapping(values.as_ptr(), out_xyz_m_s, values.len()) };
    true
}

fn frame_from_code(code: u32) -> Option<ReferenceFrame> {
    match code {
        0 => Some(ReferenceFrame::ICRF),
        1 => Some(ReferenceFrame::GCRF),
        2 => Some(ReferenceFrame::EME2000),
        3 => Some(ReferenceFrame::ITRF2020),
        4 => Some(ReferenceFrame::TEME),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_constructor_rejects_invalid_mass() {
        assert!(spacecraft_state_new(
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0,
            1.0, 1, 1,
        )
        .is_null());
    }

    #[test]
    fn ffi_round_trip_uses_explicit_si_values() {
        let state = spacecraft_state_new(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 10.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0,
            1.0, 1, 1,
        );
        assert!(!state.is_null());
        let mut output = [0.0; 3];
        // SAFETY: `state` is live and `output` contains three writable f64 values.
        assert!(unsafe { spacecraft_state_get_position_m(state, output.as_mut_ptr()) });
        assert_eq!(output, [1.0, 2.0, 3.0]);
        // SAFETY: `state` is live and has not yet been freed.
        unsafe { spacecraft_state_free(state) };
    }
}
