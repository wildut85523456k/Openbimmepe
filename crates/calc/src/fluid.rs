/// What the fluid is: density, dynamic viscosity and specific heat. These are physical properties at the design
/// temperature, settings the caller can change; the defaults are for water at about 20 degrees and air at 20 degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fluid {
    /// kg/m3
    pub density: f64,
    /// Pa.s
    pub viscosity: f64,
    /// J/(kg.K)
    pub specific_heat: f64,
}

impl Fluid {
    pub const WATER: Fluid = Fluid { density: 998.0, viscosity: 1.0e-3, specific_heat: 4186.0 };
    pub const AIR: Fluid = Fluid { density: 1.2, viscosity: 1.81e-5, specific_heat: 1006.0 };

    pub fn valid(&self) -> bool {
        [self.density, self.viscosity, self.specific_heat].iter().all(|v| v.is_finite() && *v > 0.0)
    }
}

/// The volume flow (m3/s) that carries `power` watts across a temperature difference of `delta_t` kelvin:
/// flow = power / (density x specific heat x delta_t). None when a value is not a positive finite number.
pub fn flow_from_power(power_w: f64, delta_t_k: f64, fluid: &Fluid) -> Option<f64> {
    if !(power_w.is_finite() && power_w >= 0.0 && delta_t_k.is_finite() && delta_t_k > 0.0 && fluid.valid()) { return None; }
    Some(power_w / (fluid.density * fluid.specific_heat * delta_t_k))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hundred_kilowatts_across_five_kelvin_of_water_is_about_four_point_eight_litres_a_second() {
        let q = flow_from_power(100_000.0, 5.0, &Fluid::WATER).unwrap();
        assert!((q - 100_000.0 / (998.0 * 4186.0 * 5.0)).abs() < 1e-12);
        assert!((q * 1000.0 - 4.787).abs() < 0.005, "{}", q * 1000.0);
    }

    #[test]
    fn a_kilowatt_across_ten_kelvin_of_air_is_about_eighty_three_litres_a_second() {
        let q = flow_from_power(1000.0, 10.0, &Fluid::AIR).unwrap();
        assert!((q * 1000.0 - 82.8).abs() < 0.2, "{}", q * 1000.0);
    }

    #[test]
    fn values_that_make_no_sense_give_nothing() {
        for (p, t) in [(-1.0, 5.0), (1000.0, 0.0), (1000.0, -3.0), (f64::NAN, 5.0), (1000.0, f64::INFINITY)] {
            assert_eq!(flow_from_power(p, t, &Fluid::WATER), None);
        }
        assert_eq!(flow_from_power(1000.0, 5.0, &Fluid { density: 0.0, ..Fluid::WATER }), None);
        assert_eq!(flow_from_power(0.0, 5.0, &Fluid::WATER), Some(0.0));
    }
}
