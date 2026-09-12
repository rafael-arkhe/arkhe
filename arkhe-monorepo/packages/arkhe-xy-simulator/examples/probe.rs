use arkhe_xy_simulator::diagnostics::order_parameter;
use arkhe_xy_simulator::p1_kuramoto::KuramotoConfig;
use arkhe_xy_simulator::p2_langevin::LangevinConfig;
use arkhe_xy_simulator::p2_langevin::run_langevin;
use arkhe_xy_simulator::p3_dmi::{ChiralConfig, DmiDriveSim};
use arkhe_xy_simulator::p4_pinned::{PinnedConfig, PinnedSim};

fn probe_p3() {
    // Driving dynamics: uniform is a coherent rotation (R->1, mean grad 0).
    let cfg = ChiralConfig { n: 16, k: 0.9, alpha: 0.4, dt: 0.02, seed: 7 };
    let mut s = DmiDriveSim::new(&cfg);
    let uniform = vec![0.3_f64; cfg.n * cfg.n];
    s.set_phi(&uniform);
    s.integrate(2000);
    println!("P3 uniform init: order={:.4} grad={:.6}", order_parameter(s.phi()), s.mean_grad());

    // Random init: relaxes into a fragmented basin (multi-stability), still NO
    // static pitch — falsifies the documented lambda = 2pi/alpha claim.
    let mut t = DmiDriveSim::new(&cfg);
    t.integrate(20_000);
    println!("P3 random init: order={:.4} grad={:.6}", order_parameter(t.phi()), t.mean_grad());
}

fn probe_p4() {
    let cfg = PinnedConfig {
        n: 8,
        k: 3.0,
        dt: 0.02,
        seed: 9,
        pin_sites: vec![(0usize, 0.5_f64)],
    };
    let mut s = PinnedSim::new(&cfg);
    let ph = s.phi();
    let f = s.debug_force();
    println!("P4 debug forces: nbr[1]={:?} f[1]={:.4} f[8]={:.4} f[7]={:.4}", s.debug_nbr(1), f[1], f[8], f[7]);
    let it: Vec<String> = ph.iter().enumerate().map(|(i, p)| format!("{i}:{p:.2}")).collect();
    println!("P4 init field: {}", it.chunks(8).map(|c| c.join("  ")).collect::<Vec<_>>().join("\n              "));
    s.integrate(20_000);
    let ph = s.phi();
    println!("P4 halo final: pin=0.5  nbrs={:.3} {:.3} {:.3} {:.3}", ph[1], ph[8], ph[7], ph[56]);
    let it: Vec<String> = ph.iter().enumerate().map(|(i, p)| format!("{i}:{p:.2}")).collect();
    println!("P4 final field: {}", it.chunks(8).map(|c| c.join("  ")).collect::<Vec<_>>().join("\n               "));
    println!("P4 final R={:.4} halo2={:.4} halo1={:.4}", s.order(), s.halo_align(2), s.halo_align(1));
}

fn probe_p2() {
    for t in [0.05, 0.1, 0.3] {
        let cfg = LangevinConfig {
            n: 16,
            k: 0.6,
            t,
            dt: 0.01,
            seed: 7,
        };
        let res = run_langevin(&cfg, 8_000, 500);
        println!("P2 T={t}: R {:.4} (t_kt={:.4})", res.order_mean, res.t_kt);
    }
    let _ = KuramotoConfig::default();
}

fn main() {
    probe_p3();
    probe_p4();
    probe_p2();
}