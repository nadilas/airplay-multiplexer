import { AudioDevice } from './base';
import { DeviceConfig } from '../types';

let UPnPDeviceClient: any;
try {
  UPnPDeviceClient = require('upnp-device-client');
} catch {
  // Will fail at runtime if not installed
}

export class TeufelOutputDevice extends AudioDevice {
  private client: any;

  constructor(config: DeviceConfig) {
    super(config);
    if (!config.location) {
      throw new Error(`Teufel device "${config.name}" requires a location URL from SSDP discovery`);
    }
  }

  async connect(): Promise<void> {
    try {
      this.client = new UPnPDeviceClient(this.config.location);

      // Test connection by getting device description
      await new Promise<void>((resolve, reject) => {
        this.client.getDeviceDescription((err: any, description: any) => {
          if (err) return reject(err);
          console.log(`[teufel] Connected to ${this.config.name} (${description?.friendlyName || 'unknown'})`);
          resolve();
        });
      });

      this._connected = true;
      this.emitStateChanged();
      this.emit('connected');
    } catch (err: any) {
      console.error(`[teufel] Failed to connect to ${this.config.name}: ${err.message}`);
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
      // Ignore
    }
    this.client = null;
    this._connected = false;
    this._playing = false;
    this.emitStateChanged();
    this.emit('disconnected');
  }

  async startAudio(streamUrl: string): Promise<void> {
    if (!this._connected || !this._enabled || !this.client) return;

    try {
      // Set the audio URL on the device via UPnP AVTransport
      await this.callAction('AVTransport', 'SetAVTransportURI', {
        InstanceID: 0,
        CurrentURI: streamUrl,
        CurrentURIMetaData: this.buildDIDL(streamUrl),
      });

      // Start playback
      await this.callAction('AVTransport', 'Play', {
        InstanceID: 0,
        Speed: '1',
      });

      this._playing = true;
      this.emitStateChanged();
      console.log(`[teufel] ${this.config.name} started playing from ${streamUrl}`);
    } catch (err: any) {
      console.error(`[teufel] ${this.config.name} failed to start audio: ${err.message}`);
      this._playing = false;
      this.emitStateChanged();
      this.emit('error', err);
    }
  }

  async stopAudio(): Promise<void> {
    if (!this._connected || !this.client) return;
    try {
      await this.callAction('AVTransport', 'Stop', {
        InstanceID: 0,
      });
      this._playing = false;
      this.emitStateChanged();
      console.log(`[teufel] ${this.config.name} stopped`);
    } catch (err: any) {
      console.error(`[teufel] ${this.config.name} failed to stop: ${err.message}`);
    }
  }

  async setVolume(volume: number): Promise<void> {
    await super.setVolume(volume);
    if (!this._connected || !this.client) return;
    try {
      await this.callAction('RenderingControl', 'SetVolume', {
        InstanceID: 0,
        Channel: 'Master',
        DesiredVolume: this._volume,
      });
    } catch (err: any) {
      console.error(`[teufel] ${this.config.name} failed to set volume: ${err.message}`);
    }
  }

  async setMute(muted: boolean): Promise<void> {
    await super.setMute(muted);
    if (!this._connected || !this.client) return;
    try {
      await this.callAction('RenderingControl', 'SetMute', {
        InstanceID: 0,
        Channel: 'Master',
        DesiredMute: this._muted ? '1' : '0',
      });
    } catch (err: any) {
      console.error(`[teufel] ${this.config.name} failed to set mute: ${err.message}`);
    }
  }

  private callAction(service: string, action: string, params: Record<string, any>): Promise<any> {
    return new Promise((resolve, reject) => {
      this.client.callAction(
        `urn:upnp-org:serviceId:${service}`,
        action,
        params,
        (err: any, result: any) => {
          if (err) return reject(err);
          resolve(result);
        }
      );
    });
  }

  private buildDIDL(url: string): string {
    return [
      '<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/"',
      ' xmlns:dc="http://purl.org/dc/elements/1.1/"',
      ' xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/">',
      '<item id="0" parentID="-1" restricted="1">',
      '<dc:title>Multi-Room Audio Stream</dc:title>',
      '<upnp:class>object.item.audioItem.musicTrack</upnp:class>',
      `<res protocolInfo="http-get:*:audio/wav:*">${url}</res>`,
      '</item>',
      '</DIDL-Lite>',
    ].join('');
  }
}
