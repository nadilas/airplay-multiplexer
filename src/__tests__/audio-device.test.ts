import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import { AudioDevice } from '../devices/base';
import { DeviceConfig, DeviceState } from '../types';

// Concrete test implementation of the abstract class
class TestAudioDevice extends AudioDevice {
  public connectCalled = false;
  public disconnectCalled = false;
  public startAudioUrl: string | null = null;
  public stopAudioCalled = false;

  async connect(): Promise<void> {
    this.connectCalled = true;
    this._connected = true;
    this.emitStateChanged();
    this.emit('connected');
  }

  async disconnect(): Promise<void> {
    this.disconnectCalled = true;
    this._connected = false;
    this._playing = false;
    this.emitStateChanged();
    this.emit('disconnected');
  }

  async startAudio(streamUrl: string): Promise<void> {
    this.startAudioUrl = streamUrl;
    this._playing = true;
    this.emitStateChanged();
  }

  async stopAudio(): Promise<void> {
    this.stopAudioCalled = true;
    this._playing = false;
    this.emitStateChanged();
  }
}

const testConfig: DeviceConfig = {
  id: 'test-device-1',
  name: 'Test Speaker',
  host: '192.168.1.100',
  port: 1400,
  type: 'sonos',
};

describe('AudioDevice (base class)', () => {
  let device: TestAudioDevice;

  beforeEach(() => {
    device = new TestAudioDevice(testConfig);
  });

  it('stores config and exposes id and name', () => {
    assert.strictEqual(device.id, 'test-device-1');
    assert.strictEqual(device.name, 'Test Speaker');
    assert.deepStrictEqual(device.config, testConfig);
  });

  it('has correct default state', () => {
    const state = device.getState();
    assert.strictEqual(state.volume, 50);
    assert.strictEqual(state.muted, false);
    assert.strictEqual(state.enabled, true);
    assert.strictEqual(state.connected, false);
    assert.strictEqual(state.playing, false);
  });

  it('setVolume clamps between 0 and 100', async () => {
    await device.setVolume(75);
    assert.strictEqual(device.getState().volume, 75);

    await device.setVolume(-10);
    assert.strictEqual(device.getState().volume, 0);

    await device.setVolume(150);
    assert.strictEqual(device.getState().volume, 100);
  });

  it('setVolume emits stateChanged', async () => {
    let emitted = false;
    device.on('stateChanged', () => {
      emitted = true;
    });

    await device.setVolume(60);
    assert.strictEqual(emitted, true);
  });

  it('setMute updates muted state', async () => {
    await device.setMute(true);
    assert.strictEqual(device.getState().muted, true);

    await device.setMute(false);
    assert.strictEqual(device.getState().muted, false);
  });

  it('setMute emits stateChanged', async () => {
    let emitted = false;
    device.on('stateChanged', () => {
      emitted = true;
    });

    await device.setMute(true);
    assert.strictEqual(emitted, true);
  });

  it('setEnabled updates enabled state', () => {
    device.setEnabled(false);
    assert.strictEqual(device.getState().enabled, false);

    device.setEnabled(true);
    assert.strictEqual(device.getState().enabled, true);
  });

  it('setEnabled emits stateChanged', () => {
    let emitted = false;
    device.on('stateChanged', () => {
      emitted = true;
    });

    device.setEnabled(false);
    assert.strictEqual(emitted, true);
  });

  it('connect updates connected state', async () => {
    await device.connect();
    assert.strictEqual(device.connectCalled, true);
    assert.strictEqual(device.getState().connected, true);
  });

  it('connect emits connected event', async () => {
    let emitted = false;
    device.on('connected', () => {
      emitted = true;
    });

    await device.connect();
    assert.strictEqual(emitted, true);
  });

  it('disconnect updates state', async () => {
    await device.connect();
    await device.disconnect();
    assert.strictEqual(device.disconnectCalled, true);
    assert.strictEqual(device.getState().connected, false);
    assert.strictEqual(device.getState().playing, false);
  });

  it('startAudio records the stream URL', async () => {
    const url = 'http://192.168.1.1:5000/audio/stream';
    await device.startAudio(url);
    assert.strictEqual(device.startAudioUrl, url);
    assert.strictEqual(device.getState().playing, true);
  });

  it('stopAudio updates playing state', async () => {
    await device.startAudio('http://example.com/stream');
    await device.stopAudio();
    assert.strictEqual(device.stopAudioCalled, true);
    assert.strictEqual(device.getState().playing, false);
  });

  it('getState returns a snapshot of current state', async () => {
    await device.setVolume(80);
    await device.setMute(true);
    device.setEnabled(false);

    const state = device.getState();
    assert.deepStrictEqual(state, {
      volume: 80,
      muted: true,
      enabled: false,
      connected: false,
      playing: false,
    });
  });
});
