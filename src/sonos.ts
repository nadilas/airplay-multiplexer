import { Sonos } from 'sonos';
import { Stream } from 'stream';

import {
  AudioDevice,
  DeviceConfig,
} from './models';

export class SonosDevice extends AudioDevice {
    private device: Sonos;
  
    constructor(config: DeviceConfig) {
      super(config.name);
      this.device = new Sonos(config.host, config.port);
      this.initializeDevice();
    }
  
    private async initializeDevice() {
      try {
        const deviceInfo = await this.device.getMusicLibrary("queue");
        this.isConnected = true;
        this.emit("connected");
      } catch (error) {
        console.error(`Failed to initialize Sonos device ${this.name}:`, error.message);
        this.emit("error", error);
      }
    }
  
    async setVolume(volume: number): Promise<void> {
      try {
        await super.setVolume(volume);
        await this.device.setVolume(this.volume);
      } catch (error) {
        console.error(
          `Failed to set volume on Sonos device ${this.name}:`,
          error
        );
        this.emit("error", error);
      }
    }
  
    async play(stream: Stream): Promise<void> {
      try {
        await this.device.play(stream);
        this.isPlaying = true;
        this.emit("playing");
      } catch (error) {
        console.error(`Failed to play on Sonos device ${this.name}:`, error.message);
        this.emit("error", error);
      }
    }
  
    async stop(): Promise<void> {
      try {
        await this.device.stop();
        this.isPlaying = false;
        this.emit("stopped");
      } catch (error) {
        console.error(`Failed to stop Sonos device ${this.name}:`, error.message);
        this.emit("error", error);
      }
    }
  }