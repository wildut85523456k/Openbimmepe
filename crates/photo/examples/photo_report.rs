//! Reads the photometric files given on the command line and prints one line each: what the file says and what the table adds up to.
use openbim_photo::import;

fn main() {
    for path in std::env::args().skip(1) {
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        let bytes = std::fs::read(&path).expect("file");
        for it in import(&bytes, &name) {
        let name = it.name;
        match it.result {
            Ok(p) => println!(
                "OK\t{}\t{:?}\t{}\t{}\t{:?}\t{:?}\t{:.3}\t{:?}\t{:.3}\t{:.2}\t{:?}\t{}",
                name, p.format, p.manufacturer.as_deref().unwrap_or("-"), p.catalogue.as_deref().unwrap_or("-"), p.lamp_flux_lm, p.watts,
                p.flux_lm().unwrap_or(f64::NAN), p.lor(), p.max_intensity(), p.beam_angle_deg().unwrap_or(f64::NAN), p.symmetry, p.notes.join(" | ")
            ),
            Err(e) => println!("ERR\t{name}\t{e}"),
        }
        }
    }
}
