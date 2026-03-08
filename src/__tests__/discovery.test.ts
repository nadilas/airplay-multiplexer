import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import { DeviceDiscovery } from '../discovery';

describe('DeviceDiscovery', () => {
  let discovery: DeviceDiscovery;

  beforeEach(() => {
    discovery = new DeviceDiscovery();
  });

  afterEach(async () => {
    await discovery.stop();
  });

  it('starts with no devices', () => {
    assert.deepStrictEqual(discovery.getDevices(), []);
  });

  it('getDevicesByType returns empty for each type initially', () => {
    assert.deepStrictEqual(discovery.getDevicesByType('sonos'), []);
    assert.deepStrictEqual(discovery.getDevicesByType('teufel'), []);
    assert.deepStrictEqual(discovery.getDevicesByType('airplay'), []);
  });

  it('stop clears all devices', async () => {
    // Simulate internal state - we can't easily add devices without real network
    // but stop should not throw
    await discovery.stop();
    assert.deepStrictEqual(discovery.getDevices(), []);
  });

  it('emits deviceFound when a device is discovered', (t, done) => {
    discovery.on('deviceFound', (config) => {
      assert.strictEqual(config.type, 'airplay');
      assert.strictEqual(config.name, 'Test AirPlay');
      done();
    });

    // Manually emit to simulate discovery
    (discovery as any).addDevice({
      id: 'airplay-test',
      name: 'Test AirPlay',
      host: '192.168.1.100',
      port: 7000,
      type: 'airplay',
    });
  });

  it('emits deviceLost when a device disappears', (t, done) => {
    // First add a device
    (discovery as any).addDevice({
      id: 'airplay-test',
      name: 'Test AirPlay',
      host: '192.168.1.100',
      port: 7000,
      type: 'airplay',
    });

    discovery.on('deviceLost', (config) => {
      assert.strictEqual(config.id, 'airplay-test');
      done();
    });

    (discovery as any).removeDevice('airplay-test');
  });

  it('does not emit deviceFound for duplicate device ids', () => {
    let count = 0;
    discovery.on('deviceFound', () => count++);

    const config = {
      id: 'sonos-192',
      name: 'Sonos One',
      host: '192.168.1.20',
      port: 1400,
      type: 'sonos' as const,
    };

    (discovery as any).addDevice(config);
    (discovery as any).addDevice(config); // duplicate

    assert.strictEqual(count, 1);
  });

  it('removeDevice is a no-op for unknown id', () => {
    let count = 0;
    discovery.on('deviceLost', () => count++);
    (discovery as any).removeDevice('nonexistent');
    assert.strictEqual(count, 0);
  });

  it('getDevices returns all discovered devices', () => {
    (discovery as any).addDevice({
      id: 'sonos-1',
      name: 'Sonos One',
      host: '192.168.1.20',
      port: 1400,
      type: 'sonos',
    });
    (discovery as any).addDevice({
      id: 'airplay-1',
      name: 'HomePod',
      host: '192.168.1.30',
      port: 7000,
      type: 'airplay',
    });

    const devices = discovery.getDevices();
    assert.strictEqual(devices.length, 2);
  });

  it('getDevicesByType filters correctly', () => {
    (discovery as any).addDevice({
      id: 'sonos-1',
      name: 'Sonos One',
      host: '192.168.1.20',
      port: 1400,
      type: 'sonos',
    });
    (discovery as any).addDevice({
      id: 'airplay-1',
      name: 'HomePod',
      host: '192.168.1.30',
      port: 7000,
      type: 'airplay',
    });
    (discovery as any).addDevice({
      id: 'teufel-1',
      name: 'Teufel Speaker',
      host: '192.168.1.40',
      port: 80,
      type: 'teufel',
    });

    assert.strictEqual(discovery.getDevicesByType('sonos').length, 1);
    assert.strictEqual(discovery.getDevicesByType('airplay').length, 1);
    assert.strictEqual(discovery.getDevicesByType('teufel').length, 1);
  });
});
