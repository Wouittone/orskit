package org.orskit.reference;

import java.util.Locale;

import org.hipparchus.geometry.euclidean.threed.Vector3D;
import org.orekit.frames.Frame;
import org.orekit.frames.FramesFactory;
import org.orekit.orbits.KeplerianOrbit;
import org.orekit.orbits.PositionAngleType;
import org.orekit.propagation.SpacecraftState;
import org.orekit.propagation.analytical.KeplerianPropagator;
import org.orekit.time.AbsoluteDate;
import org.orekit.utils.PVCoordinates;

/** Black-box Orekit reference for the orskit two-body comparison fixture. */
public final class TwoBodyReference {
    private static final double MU = 3.986004418e14;
    private static final double ELAPSED_SECONDS = 3600.0;

    private TwoBodyReference() {
    }

    public static void main(String[] args) {
        Frame frame = FramesFactory.getGCRF();
        AbsoluteDate epoch = AbsoluteDate.J2000_EPOCH;
        KeplerianOrbit initial = new KeplerianOrbit(
                7_200_000.0,
                0.1,
                0.7,
                0.4,
                1.1,
                2.0,
                PositionAngleType.TRUE,
                frame,
                epoch,
                MU);
        KeplerianPropagator propagator = new KeplerianPropagator(initial, MU);
        SpacecraftState propagated = propagator.propagate(epoch.shiftedBy(ELAPSED_SECONDS));
        PVCoordinates pv = propagated.getPVCoordinates(frame);
        printVector("position_m", pv.getPosition());
        printVector("velocity_m_s", pv.getVelocity());
    }

    private static void printVector(String label, Vector3D vector) {
        System.out.printf(
                Locale.ROOT,
                "%s=%.17e,%.17e,%.17e%n",
                label,
                vector.getX(),
                vector.getY(),
                vector.getZ());
    }
}
