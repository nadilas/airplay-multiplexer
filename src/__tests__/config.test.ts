import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';
import { loadConfig, AppConfig } from '../config';

describe('loadConfig', () => {
  const originalEnv = { ...process.env };

  afterEach(() => {
    // Restore original environment
    for (const key of ['RECEIVER_NAME', 'HTTP_PORT', 'SAMPLE_RATE', 'BIT_DEPTH', 'CHANNELS', 'SHAIRPORT_PATH', 'LOCAL_IP']) {
      if (originalEnv[key] === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = originalEnv[key];
      }
    }
  });

  it('returns defaults when no env vars are set', () => {
    delete process.env.RECEIVER_NAME;
    delete process.env.HTTP_PORT;
    delete process.env.SAMPLE_RATE;
    delete process.env.BIT_DEPTH;
    delete process.env.CHANNELS;
    delete process.env.SHAIRPORT_PATH;
    delete process.env.LOCAL_IP;

    const config = loadConfig();

    assert.strictEqual(config.receiverName, 'Multi-Room Audio');
    assert.strictEqual(config.httpPort, 5000);
    assert.strictEqual(config.audioFormat.sampleRate, 44100);
    assert.strictEqual(config.audioFormat.bitDepth, 16);
    assert.strictEqual(config.audioFormat.channels, 2);
    assert.strictEqual(config.shairportPath, 'shairport-sync');
    // localIp will be auto-detected, should be a string
    assert.strictEqual(typeof config.localIp, 'string');
  });

  it('reads RECEIVER_NAME from env', () => {
    process.env.RECEIVER_NAME = 'My Audio Hub';
    const config = loadConfig();
    assert.strictEqual(config.receiverName, 'My Audio Hub');
  });

  it('reads HTTP_PORT from env', () => {
    process.env.HTTP_PORT = '8080';
    const config = loadConfig();
    assert.strictEqual(config.httpPort, 8080);
  });

  it('reads audio format from env', () => {
    process.env.SAMPLE_RATE = '48000';
    process.env.BIT_DEPTH = '24';
    process.env.CHANNELS = '1';
    const config = loadConfig();
    assert.strictEqual(config.audioFormat.sampleRate, 48000);
    assert.strictEqual(config.audioFormat.bitDepth, 24);
    assert.strictEqual(config.audioFormat.channels, 1);
  });

  it('reads SHAIRPORT_PATH from env', () => {
    process.env.SHAIRPORT_PATH = '/usr/local/bin/shairport-sync';
    const config = loadConfig();
    assert.strictEqual(config.shairportPath, '/usr/local/bin/shairport-sync');
  });

  it('reads LOCAL_IP from env', () => {
    process.env.LOCAL_IP = '10.0.0.5';
    const config = loadConfig();
    assert.strictEqual(config.localIp, '10.0.0.5');
  });
});
