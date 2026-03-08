import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import { DeviceConfig } from '../types';
import { TeufelOutputDevice } from '../devices/teufel';
import { AirplayOutputDevice } from '../devices/airplay';
import { PassThrough } from 'stream';

describe('TeufelOutputDevice', () => {
  it('requires a location URL in config', () => {
    const config: DeviceConfig = {
      id: 'teufel-1',
      name: 'Teufel Speaker',
      host: '192.168.1.50',
      port: 80,
      type: 'teufel',
      // no location
    };

    assert.throws(
      () => new TeufelOutputDevice(config),
      { message: /requires a location URL/ }
    );
  });

  it('accepts config with location', () => {
    const config: DeviceConfig = {
      id: 'teufel-1',
      name: 'Teufel Speaker',
      host: '192.168.1.50',
      port: 80,
      type: 'teufel',
      location: 'http://192.168.1.50:80/description.xml',
    };

    const device = new TeufelOutputDevice(config);
    assert.strictEqual(device.id, 'teufel-1');
    assert.strictEqual(device.name, 'Teufel Speaker');
  });

  it('has correct default state', () => {
    const config: DeviceConfig = {
      id: 'teufel-1',
      name: 'Teufel Speaker',
      host: '192.168.1.50',
      port: 80,
      type: 'teufel',
      location: 'http://192.168.1.50:80/description.xml',
    };

    const device = new TeufelOutputDevice(config);
    const state = device.getState();
    assert.strictEqual(state.connected, false);
    assert.strictEqual(state.playing, false);
    assert.strictEqual(state.volume, 50);
    assert.strictEqual(state.enabled, true);
  });

  it('startAudio is a no-op when not connected', async () => {
    const config: DeviceConfig = {
      id: 'teufel-1',
      name: 'Teufel Speaker',
      host: '192.168.1.50',
      port: 80,
      type: 'teufel',
      location: 'http://192.168.1.50:80/description.xml',
    };

    const device = new TeufelOutputDevice(config);
    // Should not throw
    await device.startAudio('http://example.com/stream');
    assert.strictEqual(device.getState().playing, false);
  });

  it('stopAudio is a no-op when not connected', async () => {
    const config: DeviceConfig = {
      id: 'teufel-1',
      name: 'Teufel Speaker',
      host: '192.168.1.50',
      port: 80,
      type: 'teufel',
      location: 'http://192.168.1.50:80/description.xml',
    };

    const device = new TeufelOutputDevice(config);
    // Should not throw
    await device.stopAudio();
  });
});

describe('AirplayOutputDevice', () => {
  const airplayConfig: DeviceConfig = {
    id: 'airplay-1',
    name: 'AirPlay Speaker',
    host: '192.168.1.60',
    port: 7000,
    type: 'airplay',
  };

  it('creates with correct config', () => {
    const device = new AirplayOutputDevice(airplayConfig);
    assert.strictEqual(device.id, 'airplay-1');
    assert.strictEqual(device.name, 'AirPlay Speaker');
  });

  it('has correct default state', () => {
    const device = new AirplayOutputDevice(airplayConfig);
    const state = device.getState();
    assert.strictEqual(state.connected, false);
    assert.strictEqual(state.playing, false);
    assert.strictEqual(state.volume, 50);
    assert.strictEqual(state.enabled, true);
  });

  it('connect succeeds in HTTP fallback mode (no airtunes)', async () => {
    const device = new AirplayOutputDevice(airplayConfig);
    await device.connect();
    assert.strictEqual(device.getState().connected, true);
  });

  it('reports isRaopMode based on airtunes availability', () => {
    const device = new AirplayOutputDevice(airplayConfig);
    // Without airtunes package, should be HTTP fallback mode (not RAOP)
    assert.strictEqual(device.isRaopMode, false);
  });

  it('startAudio sets playing state when connected', async () => {
    const device = new AirplayOutputDevice(airplayConfig);
    await device.connect();
    await device.startAudio('http://example.com/stream');
    assert.strictEqual(device.getState().playing, true);
  });

  it('startAudio is a no-op when not connected', async () => {
    const device = new AirplayOutputDevice(airplayConfig);
    await device.startAudio('http://example.com/stream');
    assert.strictEqual(device.getState().playing, false);
  });

  it('startAudio is a no-op when disabled', async () => {
    const device = new AirplayOutputDevice(airplayConfig);
    await device.connect();
    device.setEnabled(false);
    await device.startAudio('http://example.com/stream');
    assert.strictEqual(device.getState().playing, false);
  });

  it('stopAudio resets playing state', async () => {
    const device = new AirplayOutputDevice(airplayConfig);
    await device.connect();
    await device.startAudio('http://example.com/stream');
    await device.stopAudio();
    assert.strictEqual(device.getState().playing, false);
  });

  it('disconnect resets state', async () => {
    const device = new AirplayOutputDevice(airplayConfig);
    await device.connect();
    await device.startAudio('http://example.com/stream');
    await device.disconnect();

    const state = device.getState();
    assert.strictEqual(state.connected, false);
    assert.strictEqual(state.playing, false);
  });

  it('pipeAudio is a no-op in HTTP fallback mode', async () => {
    const device = new AirplayOutputDevice(airplayConfig);
    await device.connect();

    const stream = new PassThrough();
    // Should not throw even without airtunes
    device.pipeAudio(stream);
    // In HTTP fallback, playing should not be set by pipeAudio
    assert.strictEqual(device.getState().playing, false);
    stream.destroy();
  });

  it('emits connected event on connect', async () => {
    const device = new AirplayOutputDevice(airplayConfig);
    let connected = false;
    device.on('connected', () => { connected = true; });
    await device.connect();
    assert.strictEqual(connected, true);
  });

  it('emits disconnected event on disconnect', async () => {
    const device = new AirplayOutputDevice(airplayConfig);
    await device.connect();

    let disconnected = false;
    device.on('disconnected', () => { disconnected = true; });
    await device.disconnect();
    assert.strictEqual(disconnected, true);
  });
});
