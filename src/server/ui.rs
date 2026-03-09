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
    .container { max-width: 900px; margin: 0 auto; padding: 20px; }
    header {
      text-align: center;
      padding: 24px 0;
      border-bottom: 1px solid #2a2a2a;
      margin-bottom: 24px;
    }
    header h1 { font-size: 24px; font-weight: 600; color: #fff; }

    /* Room tabs */
    .room-tabs {
      display: flex;
      gap: 4px;
      margin-bottom: 24px;
      overflow-x: auto;
      padding-bottom: 4px;
      align-items: center;
    }
    .room-tab {
      padding: 8px 16px;
      border-radius: 8px;
      background: #1a1a1a;
      color: #999;
      cursor: pointer;
      font-size: 14px;
      font-weight: 500;
      white-space: nowrap;
      border: 1px solid transparent;
      transition: all 0.15s;
    }
    .room-tab:hover { background: #222; color: #ccc; }
    .room-tab.active { background: #1db954; color: #fff; border-color: #1db954; }
    .room-tab.default-room::before { content: ''; display: inline-block; width: 6px; height: 6px; border-radius: 50%; background: #1db954; margin-right: 6px; }
    .room-tab.active.default-room::before { background: #fff; }
    .add-room-btn {
      padding: 8px 14px;
      border-radius: 8px;
      background: #2a2a2a;
      color: #999;
      cursor: pointer;
      font-size: 16px;
      border: 1px dashed #444;
      transition: all 0.15s;
    }
    .add-room-btn:hover { background: #333; color: #fff; border-color: #666; }

    .status-badge {
      display: inline-block;
      padding: 4px 12px;
      border-radius: 12px;
      font-size: 12px;
      font-weight: 500;
      margin-left: 8px;
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

    .section-header {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-bottom: 12px;
    }
    .section-title {
      font-size: 14px;
      font-weight: 500;
      color: #999;
      text-transform: uppercase;
      letter-spacing: 1px;
    }
    .room-actions { display: flex; gap: 8px; }

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
    .btn-unassign { background: #2a2a2a; color: #e0e0e0; font-size: 11px; }
    .btn-unassign:hover { background: #f44336; color: #fff; }
    .btn-delete { background: #2a2a2a; color: #e0e0e0; }
    .btn-delete:hover { background: #f44336; color: #fff; }
    .btn-assign { background: #1a3a1a; color: #4caf50; }
    .btn-assign:hover { background: #1db954; color: #fff; }

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

    .unassigned-section {
      margin-top: 32px;
      padding-top: 24px;
      border-top: 1px solid #2a2a2a;
    }

    .assign-dropdown {
      background: #2a2a2a;
      color: #e0e0e0;
      border: 1px solid #444;
      border-radius: 6px;
      padding: 4px 8px;
      font-size: 12px;
      cursor: pointer;
    }

    /* Modal */
    .modal-overlay {
      display: none;
      position: fixed;
      top: 0; left: 0; right: 0; bottom: 0;
      background: rgba(0,0,0,0.7);
      z-index: 1000;
      align-items: center;
      justify-content: center;
    }
    .modal-overlay.active { display: flex; }
    .modal {
      background: #1a1a1a;
      border-radius: 16px;
      padding: 24px;
      min-width: 300px;
      max-width: 400px;
    }
    .modal h3 { color: #fff; margin-bottom: 16px; font-size: 18px; }
    .modal input[type="text"] {
      width: 100%;
      padding: 10px 14px;
      border-radius: 8px;
      border: 1px solid #444;
      background: #0f0f0f;
      color: #fff;
      font-size: 14px;
      outline: none;
    }
    .modal input[type="text"]:focus { border-color: #1db954; }
    .modal-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 16px; }
    .modal-btn {
      padding: 8px 16px;
      border-radius: 8px;
      border: none;
      cursor: pointer;
      font-size: 14px;
      font-weight: 500;
    }
    .modal-btn.cancel { background: #2a2a2a; color: #999; }
    .modal-btn.confirm { background: #1db954; color: #fff; }

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
    </header>

    <div class="room-tabs" id="roomTabs"></div>

    <div id="roomPanel"></div>

    <div class="unassigned-section" id="unassignedSection" style="display:none;">
      <div class="section-header">
        <span class="section-title">Unassigned Devices</span>
      </div>
      <div id="unassignedList"></div>
    </div>
  </div>

  <!-- Create Room Modal -->
  <div class="modal-overlay" id="createRoomModal">
    <div class="modal">
      <h3>Create Room</h3>
      <input type="text" id="newRoomName" placeholder="Room name (e.g. Kitchen)">
      <div class="modal-actions">
        <button class="modal-btn cancel" onclick="closeCreateModal()">Cancel</button>
        <button class="modal-btn confirm" onclick="submitCreateRoom()">Create</button>
      </div>
    </div>
  </div>

  <script>
    let systemStatus = { rooms: [], unassignedDevices: [] };
    let activeRoomId = null;

    // SSE connection
    const evtSource = new EventSource('/api/events');
    evtSource.onmessage = function(event) {
      const data = JSON.parse(event.data);
      if (data.type === 'systemStatus') {
        systemStatus = data;
        if (!activeRoomId && data.rooms.length > 0) {
          activeRoomId = data.rooms[0].id;
        }
        renderAll();
      }
    };
    evtSource.onerror = function() {
      // Will auto-reconnect
    };

    function renderAll() {
      renderRoomTabs();
      renderRoomPanel();
      renderUnassigned();
    }

    function renderRoomTabs() {
      const container = document.getElementById('roomTabs');
      let html = '';
      for (const room of systemStatus.rooms) {
        const active = room.id === activeRoomId ? 'active' : '';
        const def = room.isDefault ? 'default-room' : '';
        html += '<div class="room-tab ' + active + ' ' + def + '" onclick="selectRoom(\'' + room.id + '\')">' +
          escapeHtml(room.name) + '</div>';
      }
      html += '<div class="add-room-btn" onclick="openCreateModal()">+</div>';
      container.innerHTML = html;
    }

    function renderRoomPanel() {
      const container = document.getElementById('roomPanel');
      const room = systemStatus.rooms.find(r => r.id === activeRoomId);
      if (!room) {
        container.innerHTML = '<div class="empty-state">No rooms configured</div>';
        return;
      }

      let html = '';

      // Room header with status + delete
      html += '<div class="section-header">';
      html += '<div><span class="section-title">' + escapeHtml(room.name) + '</span>';
      if (room.streaming) {
        html += '<span class="status-badge streaming">Streaming</span>';
      } else if (room.receiverRunning) {
        html += '<span class="status-badge idle">Ready</span>';
      } else {
        html += '<span class="status-badge idle">Offline</span>';
      }
      html += '</div>';
      html += '<div class="room-actions">';
      if (!room.isDefault) {
        html += '<button class="btn btn-delete" onclick="deleteRoom(\'' + room.id + '\')">Delete Room</button>';
      }
      html += '</div></div>';

      // Now playing
      if (room.metadata && (room.metadata.title || room.metadata.artist)) {
        html += '<div class="now-playing">';
        html += '<div class="title">' + escapeHtml(room.metadata.title || 'Unknown Track') + '</div>';
        html += '<div class="artist">' + escapeHtml(room.metadata.artist || 'Unknown Artist') + '</div>';
        if (room.metadata.album) {
          html += '<div class="album">' + escapeHtml(room.metadata.album) + '</div>';
        }
        html += '</div>';
      } else if (room.streaming) {
        html += '<div class="now-playing"><div class="title">Audio Stream</div></div>';
      }

      // Master volume
      html += '<div class="master-volume">';
      html += '<label>Master Volume</label>';
      html += '<div class="slider-row">';
      html += '<input type="range" min="0" max="100" value="' + room.masterVolume + '" ' +
        'onchange="setRoomMasterVolume(\'' + room.id + '\', this.value)" ' +
        'oninput="this.nextElementSibling.textContent=this.value">';
      html += '<span class="volume-value">' + room.masterVolume + '</span>';
      html += '</div></div>';

      // Devices
      if (room.devices.length === 0) {
        html += '<div class="empty-state">No devices assigned to this room</div>';
      } else {
        html += '<div class="section-header"><span class="section-title">Devices</span></div>';
        html += room.devices.map(d => renderDeviceCard(d, room.id)).join('');
      }

      container.innerHTML = html;
    }

    function renderDeviceCard(d, roomId) {
      const enabledClass = d.enabled ? 'enabled' : 'disabled';
      const mutedClass = d.muted ? 'muted' : '';
      return '<div class="device-card ' + (d.enabled ? '' : 'disabled') + '">' +
        '<div class="device-header">' +
          '<div>' +
            '<span class="connection-dot ' + (d.connected ? 'connected' : 'disconnected') + '"></span>' +
            '<span class="device-name">' + escapeHtml(d.name) + '</span>' +
          '</div>' +
          '<div style="display:flex;gap:6px;align-items:center;">' +
            '<span class="device-type ' + d.type + '">' + d.type + '</span>' +
            '<button class="btn btn-unassign" onclick="unassignDevice(\'' + roomId + '\', \'' + d.id + '\')">Unassign</button>' +
          '</div>' +
        '</div>' +
        '<div class="device-controls">' +
          '<div class="slider-row">' +
            '<input type="range" min="0" max="100" value="' + d.volume + '" ' +
              'onchange="setRoomDeviceVolume(\'' + roomId + '\', \'' + d.id + '\', this.value)" ' +
              'oninput="this.nextElementSibling.textContent=this.value">' +
            '<span class="volume-value">' + d.volume + '</span>' +
          '</div>' +
          '<button class="btn btn-mute ' + mutedClass + '" onclick="toggleRoomDeviceMute(\'' + roomId + '\', \'' + d.id + '\', ' + !d.muted + ')">' +
            (d.muted ? 'Unmute' : 'Mute') +
          '</button>' +
          '<button class="btn btn-enable ' + enabledClass + '" onclick="toggleRoomDeviceEnable(\'' + roomId + '\', \'' + d.id + '\', ' + !d.enabled + ')">' +
            (d.enabled ? 'On' : 'Off') +
          '</button>' +
        '</div>' +
      '</div>';
    }

    function renderUnassigned() {
      const section = document.getElementById('unassignedSection');
      const container = document.getElementById('unassignedList');
      const devices = systemStatus.unassignedDevices || [];

      if (devices.length === 0) {
        section.style.display = 'none';
        return;
      }

      section.style.display = 'block';
      container.innerHTML = devices.map(d => {
        let roomOptions = systemStatus.rooms.map(r =>
          '<option value="' + r.id + '">' + escapeHtml(r.name) + '</option>'
        ).join('');

        return '<div class="device-card">' +
          '<div class="device-header">' +
            '<div>' +
              '<span class="connection-dot ' + (d.connected ? 'connected' : 'disconnected') + '"></span>' +
              '<span class="device-name">' + escapeHtml(d.name) + '</span>' +
            '</div>' +
            '<div style="display:flex;gap:6px;align-items:center;">' +
              '<span class="device-type ' + d.type + '">' + d.type + '</span>' +
              '<select class="assign-dropdown" onchange="assignDevice(\'' + d.id + '\', this.value)">' +
                '<option value="">Assign to...</option>' +
                roomOptions +
              '</select>' +
            '</div>' +
          '</div>' +
        '</div>';
      }).join('');
    }

    function escapeHtml(text) {
      const div = document.createElement('div');
      div.textContent = text;
      return div.innerHTML;
    }

    function selectRoom(id) {
      activeRoomId = id;
      renderAll();
    }

    // Room management
    function openCreateModal() {
      document.getElementById('createRoomModal').classList.add('active');
      document.getElementById('newRoomName').focus();
    }

    function closeCreateModal() {
      document.getElementById('createRoomModal').classList.remove('active');
      document.getElementById('newRoomName').value = '';
    }

    function submitCreateRoom() {
      const name = document.getElementById('newRoomName').value.trim();
      if (!name) return;
      fetch('/api/rooms', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: name })
      }).then(r => r.json()).then(data => {
        if (data.roomId) activeRoomId = data.roomId;
        closeCreateModal();
      });
    }

    function deleteRoom(roomId) {
      if (!confirm('Delete this room? Devices will be moved to unassigned.')) return;
      fetch('/api/rooms/' + roomId, { method: 'DELETE' }).then(() => {
        if (activeRoomId === roomId && systemStatus.rooms.length > 0) {
          activeRoomId = systemStatus.rooms[0].id;
        }
      });
    }

    // Device control (room-scoped)
    function setRoomDeviceVolume(roomId, deviceId, volume) {
      fetch('/api/rooms/' + roomId + '/devices/' + deviceId + '/volume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ volume: parseInt(volume) })
      });
    }

    function toggleRoomDeviceMute(roomId, deviceId, muted) {
      fetch('/api/rooms/' + roomId + '/devices/' + deviceId + '/mute', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ muted: muted })
      });
    }

    function toggleRoomDeviceEnable(roomId, deviceId, enabled) {
      fetch('/api/rooms/' + roomId + '/devices/' + deviceId + '/enable', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ enabled: enabled })
      });
    }

    function setRoomMasterVolume(roomId, volume) {
      fetch('/api/rooms/' + roomId + '/master-volume', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ volume: parseInt(volume) })
      });
    }

    function assignDevice(deviceId, roomId) {
      if (!roomId) return;
      fetch('/api/rooms/' + roomId + '/devices', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ deviceId: deviceId })
      });
    }

    function unassignDevice(roomId, deviceId) {
      fetch('/api/rooms/' + roomId + '/devices/' + deviceId, {
        method: 'DELETE'
      });
    }

    // Enter key for modal
    document.getElementById('newRoomName').addEventListener('keypress', function(e) {
      if (e.key === 'Enter') submitCreateRoom();
    });
  </script>
</body>
</html>"##;
