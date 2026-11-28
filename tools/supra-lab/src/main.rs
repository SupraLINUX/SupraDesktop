use std::{env, fs, io, path::PathBuf};

fn main() -> io::Result<()> {
    let mut args=env::args_os().skip(1);
    let first=args.next().unwrap_or_else(|| "evidence".into());
    if first=="--nested-output" {
        let helper=PathBuf::from(args.next().ok_or_else(||io::Error::other("missing helper"))?);
        let dir=PathBuf::from(args.next().ok_or_else(||io::Error::other("missing output directory"))?);
        return supra_lab::nested_output::run(&helper,&dir);
    }
    if first=="--optics-benchmark" {
        let output=PathBuf::from(args.next().unwrap_or_else(|| "optics-benchmark.csv".into()));
        return fs::write(output,supra_lab::optics_benchmark::csv());
    }
    let output = PathBuf::from(first);
    fs::create_dir_all(&output)?;
    fs::write(output.join("geometry.ppm"), supra_lab::ppm())?;
    fs::write(output.join("blur.ppm"), supra_lab::optics::blur_fixture())?;
    fs::write(output.join("corners.ppm"), supra_lab::optics::corners_fixture())?;
    fs::write(output.join("refraction.ppm"), supra_lab::refraction_fixture::ppm())?;
    fs::write(output.join("material.ppm"), supra_lab::material_fixture::ppm())?;
    fs::write(output.join("window-optics.ppm"), supra_lab::window_optics_fixture::ppm())?;
    fs::write(output.join("window-optics-metrics.json"), supra_lab::window_optics_fixture::incremental_experiment())?;
    fs::write(output.join("scene-metrics.json"), supra_lab::scene_fixture::experiment())?;
    fs::write(
        output.join("report.json"),
        format!(
            "{{\"schema_version\":1,\"fixture\":\"geometry-optics-scene-v5\",\"width\":{},\"height\":{},\"backend\":\"cpu-reference\",\"calibrated_macos\":false}}\n",
            supra_lab::WIDTH,
            supra_lab::HEIGHT
        ),
    )?;
    println!("Geometry fixture written to {}", output.display());
    Ok(())
}
