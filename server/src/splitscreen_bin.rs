use gstreamer::{
    prelude::{ElementExtManual, GstBinExtManual},
    ElementFactory, GhostPad, traits::{ElementExt, PadExt},
};
use shared::config::ServerConfig;

pub fn build_spliscreen_bin(settings: &ServerConfig) -> Result<gstreamer::Bin, anyhow::Error> {
    let bin = gstreamer::Bin::new(Some("splitscreen"));

    let caps = gstreamer::Caps::builder("video/x-raw")
        .field("height", settings.total_resolution.height)
        .field("width", settings.total_resolution.width)
        .build();

    let video_scaleconvert = ElementFactory::make("videoconertscale").build()?;
    let encoder = ElementFactory::make("x264enc")
        .property_from_str("speed-preset", "ultrafast")
        .property_from_str("tune", "zerolatency")
        .property("bitrate", 10000u32)
        .build()?;
    let payloader = ElementFactory::make("rtph264pay").build()?;

    let video_sink = ElementFactory::make("udpsink")
        .property_from_str("host", &settings.multicast_address)
        .property("port", settings.multicast_port as i32)
        .property("auto-multicast", true)
        .build()?;

    bin.add_many(&[&video_scaleconvert, &encoder, &payloader, &video_sink])?;

    video_scaleconvert.link(&encoder)?;
    encoder.link_filtered(&payloader, &caps)?;
    payloader.link(&video_sink)?;

    let ghost_sink = GhostPad::new(Some("sink"), gstreamer::PadDirection::Sink);
    bin.add_pad(&ghost_sink)?;

    let sink = video_scaleconvert.static_pad("sink").expect("ok");
    ghost_sink.link(&sink)?;

    Ok(bin)
}
