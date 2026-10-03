package org.orskit.reference;

import java.util.Locale;
import org.orekit.frames.FramesFactory;
import org.orekit.orbits.KeplerianOrbit;
import org.orekit.orbits.PositionAngleType;
import org.orekit.propagation.analytical.KeplerianPropagator;
import org.orekit.time.AbsoluteDate;
import org.hipparchus.ode.ExpandableODE;
import org.hipparchus.ode.ODEState;
import org.hipparchus.ode.OrdinaryDifferentialEquation;
import org.hipparchus.ode.nonstiff.DormandPrince853Integrator;

/** Original public-API-only black-box reference, not an integrator implementation. */
public final class LongArcReference {
    private LongArcReference() {
    }

    public static void main(String[] args) {
        // Berry/Healy (2004), pp. 353-354: period and eccentricity only.
        // Deliberately reduced to two-body dynamics, not their full force model.
        for (double[] scenario : new double[][] {{92.05, 0.001}, {607.28, 0.716}}) {
            double mu = 3.986004418e14;
            double a = Math.cbrt(mu * Math.pow(scenario[0] * 60 / (2 * Math.PI), 2));
            var epoch = AbsoluteDate.J2000_EPOCH;
            var frame = FramesFactory.getGCRF();
            var initial = new KeplerianOrbit(a, scenario[1], 0.7, 0.4, 1.1, 0,
                    PositionAngleType.TRUE, frame, epoch, mu);
            var solver = new KeplerianPropagator(initial, mu);
            for (double time : new double[] {86400, 172800, 259200}) {
                var pv = solver.propagate(epoch.shiftedBy(time)).getPVCoordinates(frame);
                System.out.printf(Locale.ROOT,
                        "period_min=%.2f,e=%.3f,time_s=%.0f,position_m=[%.17e,%.17e,%.17e],velocity_m_s=[%.17e,%.17e,%.17e]%n",
                        scenario[0], scenario[1], time,
                        pv.getPosition().getX(), pv.getPosition().getY(), pv.getPosition().getZ(),
                        pv.getVelocity().getX(), pv.getVelocity().getY(), pv.getVelocity().getZ());
            }
        }
        double mu = 3.986004418e14;
        double a = Math.cbrt(mu * Math.pow(92.05 * 60 / (2 * Math.PI), 2));
        var initial = new KeplerianOrbit(a, 0.001, 0.7, 0.4, 1.1, 0,
                PositionAngleType.TRUE, FramesFactory.getGCRF(), AbsoluteDate.J2000_EPOCH, mu);
        var pv = initial.getPVCoordinates();
        double[] y0 = {pv.getPosition().getX(), pv.getPosition().getY(), pv.getPosition().getZ(),
                pv.getVelocity().getX(), pv.getVelocity().getY(), pv.getVelocity().getZ()};
        OrdinaryDifferentialEquation ode = new OrdinaryDifferentialEquation() {
            public int getDimension() {
                return 6;
            }

            public double[] computeDerivatives(double t, double[] y) {
                double radius = Math.sqrt(y[0] * y[0] + y[1] * y[1] + y[2] * y[2]);
                double factor = -mu / (radius * radius * radius);
                return new double[] {y[3], y[4], y[5],
                        factor * y[0] - 1e-8 * y[3],
                        factor * y[1] - 1e-8 * y[4],
                        factor * y[2] - 1e-8 * y[5]};
            }
        };
        // Independent DOP853 implementation; two local-error policies assess
        // reference convergence, not the Gauss-Jackson candidate.
        for (double tolerance : new double[] {1e-7, 1e-8}) {
            double[] absolute = {tolerance, tolerance, tolerance,
                    tolerance * 1e-3, tolerance * 1e-3, tolerance * 1e-3};
            double[] relative = {1e-15, 1e-15, 1e-15, 1e-15, 1e-15, 1e-15};
            var integrator = new DormandPrince853Integrator(1e-6, 30, absolute, relative);
            double[] end = integrator.integrate(new ExpandableODE(ode), new ODEState(0, y0),
                    259200).getPrimaryState();
            System.out.printf(Locale.ROOT,
                    "linear_drag_k_s=1e-8,absolute_position_m=%.1e,time_s=259200,position_m=[%.17e,%.17e,%.17e],velocity_m_s=[%.17e,%.17e,%.17e]%n",
                    tolerance, end[0], end[1], end[2], end[3], end[4], end[5]);
        }
    }
}
