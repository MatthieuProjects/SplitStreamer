use gstreamer::{Bin, ElementFactory, GhostPad, traits::ElementExt, prelude::{GstBinExtManual, ElementExtManual}};
use anyhow::Result;
pub mod config;

// Used because in gstreamer 1.18, videoconvertscale does not exists.
pub fn build_videoconvertscale() -> Result<Bin> {
    let bin = Bin::new(None);

    let scale = ElementFactory::make("videoscale").build()?;
    let convert = ElementFactory::make("videoconvert").build()?;

    bin.add_many(&[&scale, &convert])?;
    convert.link(&scale)?;

    let sink = convert.static_pad("sink").unwrap();
    let ghost_sink = GhostPad::with_target(Some("sink"), &sink)?;
    bin.add_pad(&ghost_sink)?;

    let src = scale.static_pad("src").unwrap();
    let ghost_src = GhostPad::with_target(Some("src"), &src)?;
    bin.add_pad(&ghost_src)?;

    Ok(bin)
}
