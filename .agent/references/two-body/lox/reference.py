"""Generate the Lox black-box vector used by orskit's two-body tests."""

import lox_space as lox


epoch = lox.Time(lox.TimeScale("TAI"), 2000, 1, 1)
earth = lox.Origin("Earth")
initial = lox.Keplerian(
    epoch,
    7_200_000 * lox.m,
    0.1,
    0.7 * lox.rad,
    1.1 * lox.rad,
    0.4 * lox.rad,
    2.0 * lox.rad,
    earth,
)
propagated = lox.Vallado(initial.to_cartesian()).propagate(epoch + 3_600 * lox.seconds)

print(
    "mu_m3_s2=",
    earth.gravitational_parameter().to_m3_per_s2(),
)
print(
    "position_m=",
    tuple(
        component.to_meters()
        for component in (propagated.x, propagated.y, propagated.z)
    ),
)
print(
    "velocity_m_s=",
    tuple(
        component.to_meters_per_second()
        for component in (propagated.vx, propagated.vy, propagated.vz)
    ),
)
