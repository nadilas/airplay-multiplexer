import { AudioDevice } from './base';
import { DeviceConfig } from '../types';

// The 'sonos' package typings
let SonosDevice: any;
try {
  SonosDevice = require('sonos').Sonos;
} catch {
  // Will fail at runtime if not installed
}

export class SonosOutputDevice extends AudioDevice {
  private device: any;

  constructor(config: DeviceConfig) {
    super(config);
    this.device = new SonosDevice(config.host, config.port || 1400);
  }

  async connect(): Promise<void> {
    try {
      // Verify device is reachable by getting its zone info
      const info = await this.device.getZoneInfo();
      console.log(`[sonos] Connected to ${this.config.name} (${info.SerialNumber || 'unknown'})`);
      this._connected = true;
      this.emitStateChanged();
      this.emit('connected');
    } catch (err: any) {
      console.error(`[sonos] Failed to connect to ${this.config.name}: ${err.message}`);
      this._connected = false;
      this.emitStateChanged();
      this.emit('error', err);
      throw err;
    }
  }

  async disconnect(): Promise<void> {
    try {
      await this.stopAudio();
    } catch {
      // Ignore errors during disconnect
    }
    this._connected = false;
    this._playing = false;
    this.emitStateChanged();
    this.emit('disconnected');
  }

  async startAudio(streamUrl: string): Promise<void> {
    if (!this._connected || !this._enabled) return;

    try {
      // SetAVTransportURI with our HTTP stream URL, then play
      await this.device.setAVTransportURI(streamUrl);
      await this.device.play();
      this._playing = true;
      this.emitStateChanged();
      console.log(`[sonos] ${this.config.name} started playing from ${streamUrl}`);
    } catch (err: any) {
      console.error(`[sonos] ${this.config.name} failed to start audio: ${err.message}`);
      this._playing = false;
      this.emitStateChanged();
      this.emit('error', err);
    }
  }

  async stopAudio(): Promise<void> {
    if (!this._connected) return;
    try {
      await this.device.stop();
      this._playing = false;
      this.emitStateChanged();
      console.log(`[sonos] ${this.config.name} stopped`);
    } catch (err: any) {
      console.error(`[sonos] ${this.config.name} failed to stop: ${err.message}`);
    }
  }

  async setVolume(volume: number): Promise<void> {
    await super.setVolume(volume);
    if (!this._connected) return;
    try {
      await this.device.setVolume(this._volume);
    } catch (err: any) {
      console.error(`[sonos] ${this.config.name} failed to set volume: ${err.message}`);
    }
  }

  async setMute(muted: boolean): Promise<void> {
    await super.setMute(muted);
    if (!this._connected) return;
    try {
      await this.device.setMuted(this._muted);
    } catch (err: any) {
      console.error(`[sonos] ${this.config.name} failed to set mute: ${err.message}`);
    }
  }
}
