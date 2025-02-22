import EventEmitter from 'events';
import { Stream } from 'stream';

export interface DeviceConfig {
  name: string;
  host: string;
  port: number;
  type: "sonos" | "teufel" | "homepod";
  location?: string; // For UPNP/DLNA devices
  serviceType?: string; // For service identification
  features?: {
    airplay2?: boolean;
    audioFormats?: string[];
    bonjourId?: string;
    model?: string;
    manufacturer?: string;
    [key: string]: any;
  };
}

export class AudioDevice extends EventEmitter {
  protected volume: number = 50;
  protected isPlaying: boolean = false;
  protected isConnected: boolean = false;

  constructor(public name: string) {
    super();
  }

  async setVolume(volume: number): Promise<void> {
    this.volume = Math.max(0, Math.min(100, volume));
  }

  async play(stream: Stream): Promise<void> {
    throw new Error("Not implemented");
  }

  async stop(): Promise<void> {
    throw new Error("Not implemented");
  }

  async pause(): Promise<void> {
    throw new Error("Not implemented");
  }

  async resume(): Promise<void> {
    throw new Error("Not implemented");
  }

  getStatus(): { isPlaying: boolean; volume: number; isConnected: boolean } {
    return {
      isPlaying: this.isPlaying,
      volume: this.volume,
      isConnected: this.isConnected,
    };
  }
}
