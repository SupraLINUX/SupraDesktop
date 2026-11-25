use std::{env, fs, io, path::PathBuf};

fn main() -> io::Result<()> {
    let output = PathBuf::from(env::args_os().nth(1).unwrap_or_else(|| "evidence".into()));
    fs::create_dir_all(&output)?;
    fs::write(output.join("geometry.ppm"), supra_lab::ppm())?;
    fs::write(output.join("blur.ppm"), supra_lab::optics::blur_fixture())?;
    fs::write(output.join("corners.ppm"), supra_lab::optics::corners_fixture())?;
    fs::write(
        output.join("report.json"),
        format!(
            "{{\"schema_version\":1,\"fixture\":\"geometry-optics-v2\",\"width\":{},\"height\":{},\"backend\":\"cpu-reference\",\"calibrated_macos\":false}}\n",
            supra_lab::WIDTH,
            supra_lab::HEIGHT
        ),
    )?;
    println!("Geometry fixture written to {}", output.display());
    Ok(())
}
