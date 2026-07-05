package org.orskit.reference;

import java.util.Locale;

import org.hipparchus.geometry.euclidean.threed.Vector3D;
import org.orekit.frames.Frame;
import org.orekit.frames.FramesFactory;
import org.orekit.orbits.CartesianOrbit;
import org.orekit.propagation.SpacecraftState;
import org.orekit.propagation.analytical.KeplerianPropagator;
import org.orekit.time.AbsoluteDate;
import org.orekit.utils.PVCoordinates;

/** Timed black-box Orekit workload matching the orskit and Lox harnesses. */
public final class TwoBodyBenchmark {
    private static final double MU = 3.986004418e14;
    private static final int DEFAULT_ITERATIONS = 1_000_000;
    private static final int WARMUP_ITERATIONS = 10_000;

    private TwoBodyBenchmark() {
    }

    public static void main(String[] args) {
        int iterations = args.length == 0 ? DEFAULT_ITERATIONS : Integer.parseInt(args[0]);
        if (iterations <= 0) {
            throw new IllegalArgumentException("iterations must be positive");
        }

        Frame frame = FramesFactory.getGCRF();
        AbsoluteDate epoch = AbsoluteDate.J2000_EPOCH;
        PVCoordinates initialPv = new PVCoordinates(
                new Vector3D(
                        -6_547_737.711_811_969,
                        1_403_357.008_528_988_8,
                        3_236_397.558_481_829),
                new Vector3D(
                        -3_483.367_356_322_263,
                        -5_479.766_927_646_723,
                        -3_108.644_196_877_947_3));
        CartesianOrbit initial = new CartesianOrbit(initialPv, frame, epoch, MU);
        KeplerianPropagator propagator = new KeplerianPropagator(initial, MU);

        runQueries(propagator, frame, epoch, WARMUP_ITERATIONS);
        long started = System.nanoTime();
        double checksum = runQueries(propagator, frame, epoch, iterations);
        long elapsedNs = System.nanoTime() - started;

        System.out.printf(
                Locale.ROOT,
                "implementation=orekit iterations=%d elapsed_ns=%d checksum=%.17e%n",
                iterations,
                elapsedNs,
                checksum);
    }

    private static double runQueries(
            KeplerianPropagator propagator,
            Frame frame,
            AbsoluteDate epoch,
            int iterations) {
        double checksum = 0.0;
        for (int index = 0; index < iterations; index++) {
            double elapsedSeconds = queryOffsetSeconds(index);
            SpacecraftState state = propagator.propagate(epoch.shiftedBy(elapsedSeconds));
            PVCoordinates pv = state.getPVCoordinates(frame);
            checksum += pv.getPosition().getX() * 1.0e-6
                    + pv.getPosition().getZ() * 2.0e-6
                    + pv.getVelocity().getY() * 1.0e-3;
        }
        return checksum;
    }

    private static double queryOffsetSeconds(int index) {
        return Math.floorMod((long) index * 104_729L, 172_800L) - 86_400.0;
    }
}
