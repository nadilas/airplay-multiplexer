import { AudioDevice } from './base';
import { DeviceConfig } from '../types';
import { PassThrough } from 'stream';

/**
 * AirPlay output device using the RAOP (Remote Audio Output Protocol).
 *
 * Note: The `airtunes` npm package implements RAOP for sending audio TO AirPlay
 * speakers. If the package is not available, this device will use HTTP streaming
 * as a fallback (same as Sonos/Teufel) for AirPlay 2 devices that support it.
 */

let airtunes: any;
try {
  airtunes = require('airtunes');
} catch {
  // airtunes not available — will use HTTP fallback
}

export class AirplayOutputDevice extends AudioDevice {
  private airtunesDevice: any;
  private audioStream: PassThrough | null = null;
  private useHttpFallback: boolean;

  constructor(config: DeviceConfig) {
    super(config);
    this.useHttpFallback = !airtunes;
    if (this.useHttpFallback) {
      console.log(`[airplay] airtunes package not available, ${config.name} will use HTTP streaming fallback`);
    }
  }

  async connect(): Promise<void> {
    try {
      if (!this.useHttpFallback && airtunes) {
        // Add device to airtunes
        this.airtunesDevice = airtunes.add(this.config.host, {
          port: this.config.port || 5000,
          volume: this._volume,
        });

        await new Promise<void>((resolve, reject) => {
          const timeout = setTimeout(() => {
            reject(new Error('Connection timeout'));
          }, 10000);

          this.airtunesDevice.on('status', (status: string) => {
            if (status === 'ready') {
              clearTimeout(timeout);
              resolve();
            }
          });

          this.airtunesDevice.on('error', (err: Error) => {
            clearTimeout(timeout);
            reject(err);
          });
        });
      }

      this._connected = true;
      this.emitStateChanged();
      this.emit('connected');
      console.log(`[airplay] Connected to ${this.config.name} (${this.useHttpFallback ? 'HTTP mode' : 'RAOP mode'})`);
    } catch (err: any) {
      console.error(`[airplay] Failed to connect to ${this.config.name}: ${err.message}`);
      this._connected = false;
      this.emitStateChanged();
      this.emit('error', err);
      throw err;
    }
  }

  async disconnect(): Promise<void> {
    try {
      await this.stopAudio();
      if (this.airtunesDevice) {
        this.airtunesDevice.stop();
        this.airtunesDevice = null;
      }
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
      // For RAOP mode, the multiplexer will pipe PCM data directly
      // via the stream manager. For HTTP fallback, the device would
      // need to pull from the URL (but most AirPlay devices don't support this).
      // The multiplexer handles the distinction.
      this._playing = true;
      this.emitStateChanged();
      console.log(`[airplay] ${this.config.name} started audio`);
    } catch (err: any) {
      console.error(`[airplay] ${this.config.name} failed to start audio: ${err.message}`);
      this._playing = false;
      this.emitStateChanged();
      this.emit('error', err);
    }
  }

  async stopAudio(): Promise<void> {
    if (this.audioStream && !this.audioStream.destroyed) {
      this.audioStream.end();
      this.audioStream = null;
    }
    this._playing = false;
    this.emitStateChanged();
    console.log(`[airplay] ${this.config.name} stopped audio`);
  }

  /**
   * For RAOP mode: pipe raw PCM audio directly to the airtunes device
   */
  pipeAudio(pcmStream: PassThrough): void {
    if (!this.airtunesDevice || this.useHttpFallback) return;

    this.audioStream = pcmStream;
    pcmStream.pipe(this.airtunesDevice);
    this._playing = true;
    this.emitStateChanged();
  }

  async setVolume(volume: number): Promise<void> {
    await super.setVolume(volume);
    if (this.airtunesDevice) {
      try {
        this.airtunesDevice.setVolume(this._volume);
      } catch (err: any) {
        console.error(`[airplay] ${this.config.name} failed to set volume: ${err.message}`);
      }
    }
  }

  get isRaopMode(): boolean {
    return !this.useHttpFallback;
  }
}
