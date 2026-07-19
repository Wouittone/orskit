package org.orskit.reference;

import java.util.Locale;

import org.hipparchus.geometry.euclidean.threed.Vector3D;
import org.hipparchus.linear.MatrixUtils;
import org.hipparchus.linear.RealMatrix;
import org.orekit.estimation.measurements.ObservableSatellite;
import org.orekit.estimation.measurements.Position;
import org.orekit.estimation.sequential.ConstantProcessNoise;
import org.orekit.estimation.sequential.KalmanEstimator;
import org.orekit.estimation.sequential.KalmanEstimatorBuilder;
import org.orekit.frames.Frame;
import org.orekit.frames.FramesFactory;
import org.orekit.orbits.CartesianOrbit;
import org.orekit.orbits.Orbit;
import org.orekit.orbits.PositionAngleType;
import org.orekit.propagation.Propagator;
import org.orekit.propagation.SpacecraftState;
import org.orekit.propagation.conversion.KeplerianPropagatorBuilder;
import org.orekit.time.AbsoluteDate;
import org.orekit.utils.PVCoordinates;

/**
 * Timed, public-API-only Orekit sequential Kalman correction matching the
 * orskit Cartesian position EKF benchmark scenario.
 */
public final class CartesianPositionOdBenchmark {
    private static final double MU = 3.986_004_415e14;
    private static final int DEFAULT_ITERATIONS = 10_000;
    private static final int WARMUP_ITERATIONS = 100;

    private CartesianPositionOdBenchmark() {
    }

    public static void main(String[] args) {
        int iterations = args.length == 0 ? DEFAULT_ITERATIONS : Integer.parseInt(args[0]);
        if (iterations <= 0) {
            throw new IllegalArgumentException("iterations must be positive");
        }

        runQueries(WARMUP_ITERATIONS);
        long started = System.nanoTime();
        double checksum = runQueries(iterations);
        long elapsedNs = System.nanoTime() - started;
        System.out.printf(
                Locale.ROOT,
                "implementation=orekit-ekf-position iterations=%d elapsed_ns=%d checksum=%.17e%n",
                iterations,
                elapsedNs,
                checksum);
    }

    private static double runQueries(int iterations) {
        double checksum = 0.0;
        for (int index = 0; index < iterations; index++) {
            KalmanEstimator estimator = newEstimator();
            Propagator posterior = estimator.estimationStep(observation())[0];
            Vector3D position = posterior.getInitialState().getPosition();
            checksum += position.getX() * 1.0e-6
                    + position.getY() * 2.0e-6
                    + position.getZ() * 3.0e-6;
        }
        return checksum;
    }

    private static KalmanEstimator newEstimator() {
        RealMatrix initialCovariance = MatrixUtils.createRealIdentityMatrix(6).scalarMultiply(1.0e6);
        RealMatrix processNoise = MatrixUtils.createRealIdentityMatrix(6).scalarMultiply(1.0e-8);
        KeplerianPropagatorBuilder propagator = new KeplerianPropagatorBuilder(
                prior(), PositionAngleType.TRUE, 1.0);
        return new KalmanEstimatorBuilder()
                .addPropagationConfiguration(
                        propagator,
                        new ConstantProcessNoise(initialCovariance, processNoise))
                .build();
    }

    private static Orbit prior() {
        Frame frame = FramesFactory.getGCRF();
        AbsoluteDate epoch = AbsoluteDate.J2000_EPOCH;
        PVCoordinates pv = new PVCoordinates(
                new Vector3D(6_999_600.0, 350.0, -250.0),
                new Vector3D(0.0, 7_546.0, 0.0));
        return new CartesianOrbit(pv, frame, epoch, MU);
    }

    private static Position observation() {
        return new Position(
                AbsoluteDate.J2000_EPOCH,
                new Vector3D(7_000_010.0, -10.0, 5.0),
                5.0,
                1.0,
                new ObservableSatellite(0));
    }
}
