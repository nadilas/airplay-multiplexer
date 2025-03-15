// src/homepod.ts
import airplay from 'airplay-protocol';
import {
  PassThrough,
  Stream,
} from 'stream';

import {
  AudioDevice,
  DeviceConfig,
} from './models';

export class AirplayDevice extends AudioDevice {
  private airplayDevice: any;
  private reconnectTimeout?: NodeJS.Timeout;
  private readonly MAX_RECONNECT_ATTEMPTS = 5;
  private reconnectAttempts = 0;
  private streamBuffer?: PassThrough;

  constructor(config: DeviceConfig) {
    super(config.name);
    console.log(airplay)
    this.airplayDevice = airplay(
      config.host,
      config.port || 7000,
    );

    this.initializeDevice();
    this.setupEventHandlers();
  }

  private setupEventHandlers() {
    this.airplayDevice.on('error', this.handleConnectionError.bind(this));
    this.airplayDevice.on('disconnected', this.handleDisconnect.bind(this));
    this.airplayDevice.on('connected', () => {
      this.isConnected = true;
      this.reconnectAttempts = 0;
      this.emit("connected");
    });
  }

  private async initializeDevice() {
    try {
      await this.airplayDevice.connect();
    } catch (error) {
      console.error(`Failed to initialize HomePod ${this.name}:`, error.message);
      this.handleConnectionError(error);
    }
  }

  private handleConnectionError(error: Error) {
    console.error(`HomePod ${this.name} connection error:`, error.message);
    this.emit("error", error);
    this.isConnected = false;
    this.attemptReconnection();
  }

  private handleDisconnect() {
    console.log(`HomePod ${this.name} disconnected`);
    this.isConnected = false;
    this.attemptReconnection();
  }

  private attemptReconnection() {
    if (this.reconnectAttempts >= this.MAX_RECONNECT_ATTEMPTS) {
      console.error(`Failed to reconnect to HomePod ${this.name} after ${this.MAX_RECONNECT_ATTEMPTS} attempts`);
      this.emit("error", new Error("Max reconnection attempts reached"));
      return;
    }

    if (this.reconnectTimeout) {
      clearTimeout(this.reconnectTimeout);
    }

    this.reconnectTimeout = setTimeout(async () => {
      try {
        this.reconnectAttempts++;
        console.log(`Attempting to reconnect to HomePod ${this.name} (attempt ${this.reconnectAttempts})`);
        await this.airplayDevice.connect();
      } catch (error) {
        console.error(`Reconnection attempt failed:`, error.message);
        this.attemptReconnection();
      }
    }, Math.min(1000 * Math.pow(2, this.reconnectAttempts), 30000)); // Exponential backoff
  }

  async setVolume(volume: number): Promise<void> {
    try {
      await super.setVolume(volume);
      await this.airplayDevice.volume(this.volume / 100); // Convert to 0-1 range
    } catch (error) {
      console.error(`Failed to set volume on HomePod ${this.name}:`, error.message);
      this.emit("error", error);
    }
  }

  async play(stream: Stream): Promise<void> {
    try {
      console.log(`${this.name} starting playing on device`) 
      this.streamBuffer = new PassThrough();
      stream.pipe(this.streamBuffer);

      await this.airplayDevice.play(this.streamBuffer);
      
      this.isPlaying = true;
      this.emit("playing");
    } catch (error) {
      console.error(`Failed to play on HomePod ${this.name}:`, error.message);
      this.emit("error", error);
    }
  }

  async stop(): Promise<void> {
    try {
      await this.airplayDevice.stop();
      
      if (this.streamBuffer) {
        this.streamBuffer.end();
        this.streamBuffer = undefined;
      }

      this.isPlaying = false;
      this.emit("stopped");
    } catch (error) {
      console.error(`Failed to stop HomePod ${this.name}:`, error.message);
      this.emit("error", error);
    }
  }

  async pause(): Promise<void> {
    try {
      await this.airplayDevice.pause();
      this.isPlaying = false;
      this.emit("paused");
    } catch (error) {
      console.error(`Failed to pause HomePod ${this.name}:`, error.message);
      this.emit("error", error);
    }
  }

  async resume(): Promise<void> {
    try {
      await this.airplayDevice.resume();
      this.isPlaying = true;
      this.emit("playing");
    } catch (error) {
      console.error(`Failed to resume HomePod ${this.name}:`, error.message);
      this.emit("error", error);
    }
  }
}