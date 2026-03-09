use anyhow::Result;

pub const AV_TRANSPORT: &str = "urn:schemas-upnp-org:service:AVTransport:1";
pub const RENDERING_CONTROL: &str = "urn:schemas-upnp-org:service:RenderingControl:1";

/// Build a SOAP envelope for a UPnP action.
pub fn build_soap_envelope(service_type: &str, action: &str, args: &[(&str, &str)]) -> String {
    let mut body_args = String::new();
    for (key, value) in args {
        body_args.push_str(&format!("<{key}>{value}</{key}>"));
    }

    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"
  s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:{action} xmlns:u="{service_type}">
      {body_args}
    </u:{action}>
  </s:Body>
</s:Envelope>"#
    )
}

/// Build DIDL-Lite metadata XML for SetAVTransportURI.
pub fn build_didl_lite(stream_url: &str) -> String {
    format!(
        r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/"><item id="0" parentID="-1" restricted="1"><dc:title>Multi-Room Audio Stream</dc:title><upnp:class>object.item.audioItem.musicTrack</upnp:class><res protocolInfo="http-get:*:audio/wav:*">{stream_url}</res></item></DIDL-Lite>"#
    )
}

/// XML-escape a string for embedding in SOAP body.
pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Send a SOAP action to a UPnP control URL.
pub async fn call_action(
    client: &reqwest::Client,
    control_url: &str,
    service_type: &str,
    action: &str,
    args: &[(&str, &str)],
) -> Result<String> {
    let body = build_soap_envelope(service_type, action, args);
    let soap_action = format!("\"{}#{}\"", service_type, action);

    let response = client
        .post(control_url)
        .header("Content-Type", "text/xml; charset=utf-8")
        .header("SOAPAction", &soap_action)
        .body(body)
        .send()
        .await?;

    let text = response.text().await?;
    Ok(text)
}

/// SetAVTransportURI + Play convenience method.
pub async fn set_av_transport_and_play(
    client: &reqwest::Client,
    control_url: &str,
    stream_url: &str,
) -> Result<()> {
    let didl = build_didl_lite(stream_url);
    let escaped_didl = xml_escape(&didl);

    call_action(
        client,
        control_url,
        AV_TRANSPORT,
        "SetAVTransportURI",
        &[
            ("InstanceID", "0"),
            ("CurrentURI", stream_url),
            ("CurrentURIMetaData", &escaped_didl),
        ],
    )
    .await?;

    call_action(
        client,
        control_url,
        AV_TRANSPORT,
        "Play",
        &[("InstanceID", "0"), ("Speed", "1")],
    )
    .await?;

    Ok(())
}

/// Parse friendly name from a UPnP device description XML.
pub fn parse_friendly_name(xml: &str) -> Option<String> {
    // Use simple string matching (same approach as the original TS code)
    let start = xml.find("<friendlyName>")?;
    let after = &xml[start + "<friendlyName>".len()..];
    let end = after.find("</friendlyName>")?;
    Some(after[..end].to_string())
}

/// Check if XML indicates a Teufel/Raumfeld device.
pub fn is_teufel_device(xml: &str) -> bool {
    let lower = xml.to_lowercase();
    lower.contains("teufel") || lower.contains("raumfeld")
}

/// Check if XML indicates a Sonos device.
pub fn is_sonos_device(xml: &str) -> bool {
    let lower = xml.to_lowercase();
    lower.contains("sonos")
}

/// Check if XML indicates a MediaRenderer.
pub fn is_media_renderer(xml: &str) -> bool {
    xml.contains("MediaRenderer")
}

/// Parse AVTransport and RenderingControl control URLs from device description XML.
pub fn parse_control_urls(xml: &str, base_url: &str) -> (Option<String>, Option<String>) {
    let mut av_transport_url = None;
    let mut rendering_url = None;

    // Parse service blocks to find controlURL for each service type
    let mut search_from = 0;
    while let Some(service_start) = xml[search_from..].find("<service>") {
        let abs_start = search_from + service_start;
        let service_end = match xml[abs_start..].find("</service>") {
            Some(e) => abs_start + e + "</service>".len(),
            None => break,
        };
        let block = &xml[abs_start..service_end];

        if let Some(ctrl_url) = extract_tag(block, "controlURL") {
            let full_url = if ctrl_url.starts_with("http") {
                ctrl_url.to_string()
            } else {
                format!("{}{}", base_url.trim_end_matches('/'), ctrl_url)
            };

            if block.contains("AVTransport") {
                av_transport_url = Some(full_url);
            } else if block.contains("RenderingControl") {
                rendering_url = Some(full_url);
            }
        }

        search_from = service_end;
    }

    (av_transport_url, rendering_url)
}

fn extract_tag<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)?;
    let after = &xml[start + open.len()..];
    let end = after.find(&close)?;
    Some(&after[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_soap_envelope() {
        let envelope = build_soap_envelope(AV_TRANSPORT, "Play", &[("InstanceID", "0"), ("Speed", "1")]);
        assert!(envelope.contains("s:Envelope"));
        assert!(envelope.contains("u:Play"));
        assert!(envelope.contains(&format!("xmlns:u=\"{}\"", AV_TRANSPORT)));
        assert!(envelope.contains("<InstanceID>0</InstanceID>"));
        assert!(envelope.contains("<Speed>1</Speed>"));
    }

    #[test]
    fn test_build_didl_lite() {
        let didl = build_didl_lite("http://192.168.1.1:5000/audio/stream");
        assert!(didl.contains("DIDL-Lite"));
        assert!(didl.contains("Multi-Room Audio Stream"));
        assert!(didl.contains("object.item.audioItem.musicTrack"));
        assert!(didl.contains("http://192.168.1.1:5000/audio/stream"));
        assert!(didl.contains("http-get:*:audio/wav:*"));
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("<test>"), "&lt;test&gt;");
        assert_eq!(xml_escape("a&b"), "a&amp;b");
        assert_eq!(xml_escape("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_parse_friendly_name() {
        let xml = r#"<device><friendlyName>Living Room Speaker</friendlyName></device>"#;
        assert_eq!(parse_friendly_name(xml), Some("Living Room Speaker".to_string()));
    }

    #[test]
    fn test_parse_friendly_name_missing() {
        assert_eq!(parse_friendly_name("<device></device>"), None);
    }

    #[test]
    fn test_is_teufel_device() {
        assert!(is_teufel_device("<manufacturer>Teufel GmbH</manufacturer>"));
        assert!(is_teufel_device("<manufacturer>Raumfeld</manufacturer>"));
        assert!(!is_teufel_device("<manufacturer>Sonos Inc</manufacturer>"));
    }

    #[test]
    fn test_is_sonos_device() {
        assert!(is_sonos_device("<manufacturer>Sonos, Inc.</manufacturer>"));
        assert!(!is_sonos_device("<manufacturer>Teufel</manufacturer>"));
    }

    #[test]
    fn test_is_media_renderer() {
        assert!(is_media_renderer("<deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>"));
        assert!(!is_media_renderer("<deviceType>urn:schemas-upnp-org:device:MediaServer:1</deviceType>"));
    }

    #[test]
    fn test_parse_control_urls() {
        let xml = r#"
        <root>
            <device>
                <serviceList>
                    <service>
                        <serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>
                        <controlURL>/MediaRenderer/AVTransport/Control</controlURL>
                    </service>
                    <service>
                        <serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType>
                        <controlURL>/MediaRenderer/RenderingControl/Control</controlURL>
                    </service>
                </serviceList>
            </device>
        </root>"#;

        let (av, rc) = parse_control_urls(xml, "http://192.168.1.50:80");
        assert_eq!(
            av.unwrap(),
            "http://192.168.1.50:80/MediaRenderer/AVTransport/Control"
        );
        assert_eq!(
            rc.unwrap(),
            "http://192.168.1.50:80/MediaRenderer/RenderingControl/Control"
        );
    }
}
