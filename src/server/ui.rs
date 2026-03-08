use std::sync::Arc;

use axum::response::Html;
use axum::routing::get;
use axum::Router;

use crate::server::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/", get(dashboard))
}

async fn dashboard() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Multi-Room Audio</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      background: #0f0f0f;
      color: #e0e0e0;
      min-height: 100vh;
    }
    .container { max-width: 800px; margin: 0 auto; padding: 20px; }
    header {
      text-align: center;
      padding: 30px 0;
      border-bottom: 1px solid #2a2a2a;
      margin-bottom: 30px;
    }
    header h1 { font-size: 24px; font-weight: 600; color: #fff; }
    .status-badge {
      display: inline-block;
      padding: 4px 12px;
      border-radius: 12px;
      font-size: 12px;
      font-weight: 500;
      margin-top: 8px;
    }
    .status-badge.streaming { background: #1db954; color: #fff; }
    .status-badge.idle { background: #333; color: #999; }

    .now-playing {
      background: #1a1a1a;
      border-radius: 12px;
      padding: 20px;
      margin-bottom: 24px;
      text-align: center;
    }
    .now-playing .title { font-size: 18px; font-weight: 600; color: #fff; }
    .now-playing .artist { font-size: 14px; color: #999; margin-top: 4px; }
    .now-playing .album { font-size: 12px; color: #666; margin-top: 2px; }

    .master-volume {
      background: #1a1a1a;
      border-radius: 12px;
      padding: 20px;
      margin-bottom: 24px;
    }
    .master-volume label { font-size: 14px; font-weight: 500; color: #999; }
    .master-volume .slider-row { display: flex; align-items: center; gap: 12px; margin-top: 8px; }
    .master-volume .volume-value { font-size: 14px; color: #fff; min-width: 36px; text-align: right; }

    .devices-header {
      font-size: 14px;
      font-weight: 500;
      color: #999;
      text-transform: uppercase;
      letter-spacing: 1px;
      margin-bottom: 12px;
    }

    .device-card {
      background: #1a1a1a;
      border-radius: 12px;
      padding: 16px 20px;
      margin-bottom: 12px;
      transition: opacity 0.2s;
    }
    .device-card.disabled { opacity: 0.5; }
    .device-card .device-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 12px;
    }
    .device-card .device-name { font-size: 16px; font-weight: 500; color: #fff; }
    .device-card .device-type {
      font-size: 11px;
      padding: 2px 8px;
      border-radius: 8px;
      background: #2a2a2a;
      color: #999;
      text-transform: uppercase;
    }
    .device-card .device-type.sonos { background: #1a3a1a; color: #4caf50; }
    .device-card .device-type.teufel { background: #3a1a1a; color: #f44336; }
    .device-card .device-type.airplay { background: #1a1a3a; color: #2196f3; }

    .device-controls {
      display: flex;
      align-items: center;
      gap: 12px;
    }
    .device-controls .slider-row { flex: 1; display: flex; align-items: center; gap: 8px; }
    .device-controls .volume-value { font-size: 13px; color: #999; min-width: 30px; text-align: right; }

    input[type="range"] {
      -webkit-appearance: none;
      appearance: none;
      flex: 1;
      height: 4px;
      border-radius: 2px;
      background: #333;
      outline: none;
    }
    input[type="range"]::-webkit-slider-thumb {
      -webkit-appearance: none;
      appearance: none;
      width: 16px;
      height: 16px;
      border-radius: 50%;
      background: #fff;
      cursor: pointer;
    }

    .btn {
      border: none;
      border-radius: 6px;
      padding: 6px 12px;
      font-size: 12px;
      cursor: pointer;
      font-weight: 500;
      transition: background 0.15s;
    }
    .btn-mute { background: #2a2a2a; color: #e0e0e0; }
    .btn-mute.muted { background: #f44336; color: #fff; }
    .btn-enable { background: #2a2a2a; color: #e0e0e0; }
    .btn-enable.enabled { background: #1db954; color: #fff; }

    .connection-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      display: inline-block;
      margin-right: 6px;
    }
    .connection-dot.connected { background: #1db954; }
    .connection-dot.disconnected { background: #666; }

    .empty-state {
      text-align: center;
      padding: 40px;
      color: #666;
      font-size: 14px;
    }

    @media (max-width: 600px) {
      .container { padding: 12px; }
      .device-controls { flex-wrap: wrap; }
    }
  </style>
</head>
<body>
  <div class="container">
    <header>
      <h1>Multi-Room Audio</h1>
      <div id="receiverStatus" class="status-badge idle">Not Streaming</div>
    </header>

    <div id="nowPlaying" class="now-playing" style="display:none;">
      <div class="title" id="trackTitle">--</div>
      <div class="artist" id="trackArtist">--</div>
      <div class="album" id="trackAlbum"></div>
    </div>

    <div class="master-volume">
      <label>Master Volume</label>
      <div class="slider-row">
        <input type="range" id="masterVolume" min="0" max="100" value="50">
        <span class="volume-value" id="masterVolumeValue">50</span>
      </div>
    </div>

    <div class="devices-header">Devices</div>
    <div id="deviceList">
      <div class="empty-state">Searching for devices...</div>
    </div>
  </div>

  <script>
    let devices = [];
    let masterVolume = 50;

    // SSE connection for real-time updates
    const evtSource = new EventSource('/api/events');
    evtSource.onmessage = function(event) {
      const data = JSON.parse(event.data);
      if (data.type === 'status') {
        updateUI(data);
      }
    };
    evtSource.onerror = function() {
      document.getElementById('receiverStatus').textContent = 'Disconnected';
      document.getElementById('receiverStatus').className = 'status-badge idle';
    };

    function updateUI(status) {
      // Receiver status
      const badge = document.getElementById('receiverStatus');
      if (status.streaming) {
        badge.textContent = 'Streaming';
        badge.className = 'status-badge streaming';
      } else if (status.receiverRunning) {
        badge.textContent = 'Ready';
        badge.className = 'status-badge idle';
      } else {
        badge.textContent = 'Offline';
        badge.className = 'status-badge idle';
      }

      // Now playing
      const np = document.getElementById('nowPlaying');
      if (status.metadata && (status.metadata.title || status.metadata.artist)) {
        np.style.display = 'block';
        document.getElementById('trackTitle').textContent = status.metadata.title || 'Unknown Track';
        document.getElementById('trackArtist').textContent = status.metadata.artist || 'Unknown Artist';
        document.getElementById('trackAlbum').textContent = status.metadata.album || '';
      } else if (status.streaming) {
        np.style.display = 'block';
        document.getElementById('trackTitle').textContent = 'Audio Stream';
        document.getElementById('trackArtist').textContent = '';
        document.getElementById('trackAlbum').textContent = '';
      } else {
        np.style.display = 'none';
      }

      // Devices
      devices = status.devices || [];
      renderDevices();
    }

    function renderDevices() {
      const container = document.getElementById('deviceList');
      if (devices.length === 0) {
        container.innerHTML = '<div class="empty-state">Searching for devices...</div>';
        return;
      }

      container.innerHTML = devices.map(d => {
        const enabledClass = d.enabled ? 'enabled' : 'disabled';
        const mutedClass = d.muted ? 'muted' : '';
        return '<div class="device-card ' + (d.enabled ? '' : 'disabled') + '">' +
          '<div class="device-header">' +
            '<div>' +
              '<span class="connection-dot ' + (d.connected ? 'connected' : 'disconnected') + '"></span>' +
              '<span class="device-name">' + escapeHtml(d.name) + '</span>' +
            '</div>' +
            '<span class="device-type ' + d.type + '">' + d.type + '</span>' +
          '</div>' +
          '<div class="device-controls">' +
            '<div class="slider-row">' +
              '<input type="range" min="0" max="100" value="' + d.volume + '" ' +
                'onchange="setVolume(\'' + d.id + '\', this.value)" ' +
                'oninput="this.nextElementSibling.textContent=this.value">' +
              '<span class="volume-value">' + d.volume + '</span>' +
            '</div>' +
            '<button class="btn btn-mute ' + mutedClass + '" onclick="toggleMute(\'' + d.id + '\', ' + !d.muted + ')">' +
              (d.muted ? 'Unmute' : 'Mute') +
            '</button>' +
            '<button class="btn btn-enable ' + enabledClass + '" onclick="toggleEnable(\'' + d.id + '\', ' + !d.enabled + ')">' +
              (d.enabled ? 'On' : 'Off') +
            '</button>' +
          '</div>' +
        '</div>';
      }).join('');
    }

    function escapeHtml(text) {
      const div = document.createElement('div');
      div.textContent = text;
      return div.innerHTML;
    }

    // Master volume
    const masterSlider = document.getElementById('masterVolume');
    const masterValue = document.getElementById('masterVolumeValue');
    masterSlider.oninput = function() {
      masterValue.textContent = this.value;
    };
    masterSlider.onchange = function() {
      fetch('/api/master-volume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ volume: parseInt(this.value) })
      });
    };

    function setVolume(id, volume) {
      fetch('/api/devices/' + id + '/volume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ volume: parseInt(volume) })
      });
    }

    function toggleMute(id, muted) {
      fetch('/api/devices/' + id + '/mute', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ muted: muted })
      });
    }

    function toggleEnable(id, enabled) {
      fetch('/api/devices/' + id + '/enable', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: enabled })
      });
    }
  </script>
</body>
</html>"##;
