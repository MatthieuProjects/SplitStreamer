use gstreamer::{
    prelude::{ElementExtManual, GstBinExtManual},
    traits::ElementExt,
    ElementFactory, GhostPad,
};
use shared::config::ServerConfig;

pub fn build_spliscreen_bin(settings: &ServerConfig) -> Result<gstreamer::Bin, anyhow::Error> {
    let bin = gstreamer::Bin::new(Some("splitscreen"));

    let caps = gstreamer::Caps::builder("video/x-raw")
        .field("height", settings.total_resolution.height as i32)
        .field("width", settings.total_resolution.width as i32)
        .build();

    let video_scaleconvert = ElementFactory::make("videoconvertscale").build()?;
    let encoder = ElementFactory::make("x264enc")
        .property_from_str("speed-preset", "ultrafast")
        .property_from_str("tune", "zerolatency")
        .property("bitrate", 1000u32)
        .build()?;
    let payloader = ElementFactory::make("rtph264pay").build()?;

    let video_sink = ElementFactory::make("udpsink")
        .property_from_str("host", &settings.multicast_address)
        .property("port", settings.multicast_port as i32)
        .property("auto-multicast", true)
        .build()?;

    bin.add_many(&[&video_scaleconvert, &encoder, &payloader, &video_sink])?;

    video_scaleconvert.link_filtered(&encoder, &caps)?;
    encoder.link(&payloader)?;
    payloader.link(&video_sink)?;

    let sink = video_scaleconvert.static_pad("sink").expect("ok");
    let ghost_sink = GhostPad::with_target(Some("sink"), &sink)?;
    bin.add_pad(&ghost_sink)?;

    Ok(bin)
}
