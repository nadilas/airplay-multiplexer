import { EventEmitter } from 'events';
import { ShairportManager } from './shairport';
import { StreamManager } from './stream-manager';
import { DeviceDiscovery } from './discovery';
import { AppConfig } from './config';
import { AudioDevice } from './devices/base';
import { SonosOutputDevice } from './devices/sonos';
import { TeufelOutputDevice } from './devices/teufel';
import { AirplayOutputDevice } from './devices/airplay';
import { DeviceConfig, MultiplexerStatus, TrackMetadata } from './types';

export class AudioMultiplexer extends EventEmitter {
  private config: AppConfig;
  private shairport: ShairportManager;
  private streamManager: StreamManager;
  private discovery: DeviceDiscovery;
  private devices = new Map<string, AudioDevice>();
  private metadata: TrackMetadata = {};
  private streaming = false;
  private audioStreamUrl: string;

  constructor(config: AppConfig) {
    super();
    this.config = config;
    this.shairport = new ShairportManager(config);
    this.streamManager = new StreamManager(config.audioFormat);
    this.discovery = new DeviceDiscovery();
    this.audioStreamUrl = `http://${config.localIp}:${config.httpPort}/audio/stream`;

    this.setupShairportHandlers();
    this.setupDiscoveryHandlers();
  }

  private setupShairportHandlers(): void {
    this.shairport.on('audio', (audioStream) => {
      console.log('[multiplexer] Audio stream started from shairport-sync');
      this.streaming = true;
      this.streamManager.setSource(audioStream);

      // Start audio on all enabled, connected devices
      for (const [id, device] of this.devices) {
        if (device.getState().enabled && device.getState().connected) {
          this.startDeviceAudio(device);
        }
      }

      this.emitStatusChanged();

      audioStream.on('end', () => {
        console.log('[multiplexer] Audio stream ended');
        this.streaming = false;
        this.stopAllDeviceAudio();
        this.emitStatusChanged();
      });
    });

    this.shairport.on('metadata', (meta: TrackMetadata) => {
      this.metadata = { ...this.metadata, ...meta };
      this.emitStatusChanged();
    });

    this.shairport.on('stopped', () => {
      this.streaming = false;
      this.metadata = {};
      this.emitStatusChanged();
    });

    this.shairport.on('error', (err: Error) => {
      console.error(`[multiplexer] Shairport error: ${err.message}`);
    });
  }

  private setupDiscoveryHandlers(): void {
    this.discovery.on('deviceFound', (config: DeviceConfig) => {
      this.addDevice(config);
    });

    this.discovery.on('deviceLost', (config: DeviceConfig) => {
      this.removeDevice(config.id);
    });
  }

  private addDevice(config: DeviceConfig): void {
    if (this.devices.has(config.id)) return;

    let device: AudioDevice;

    switch (config.type) {
      case 'sonos':
        device = new SonosOutputDevice(config);
        break;
      case 'teufel':
        device = new TeufelOutputDevice(config);
        break;
      case 'airplay':
        device = new AirplayOutputDevice(config);
        break;
      default:
        console.warn(`[multiplexer] Unknown device type: ${config.type}`);
        return;
    }

    device.on('stateChanged', () => {
      this.emitStatusChanged();
    });

    device.on('error', (err: Error) => {
      console.error(`[multiplexer] Device ${config.name} error: ${err.message}`);
    });

    this.devices.set(config.id, device);
    console.log(`[multiplexer] Added device: ${config.name} (${config.type})`);

    // Auto-connect the device
    device.connect().then(() => {
      // If we're already streaming, start audio on this device
      if (this.streaming && device.getState().enabled) {
        this.startDeviceAudio(device);
      }
      this.emitStatusChanged();
    }).catch((err) => {
      console.error(`[multiplexer] Failed to connect ${config.name}: ${err.message}`);
    });

    this.emitStatusChanged();
  }

  private removeDevice(id: string): void {
    const device = this.devices.get(id);
    if (!device) return;

    this.streamManager.unsubscribe(id);
    device.disconnect().catch(() => {});
    this.devices.delete(id);
    console.log(`[multiplexer] Removed device: ${device.name}`);
    this.emitStatusChanged();
  }

  private startDeviceAudio(device: AudioDevice): void {
    const config = device.config;

    if (config.type === 'airplay' && device instanceof AirplayOutputDevice && device.isRaopMode) {
      // AirPlay RAOP mode: pipe PCM directly
      const pcmStream = this.streamManager.subscribe(config.id);
      device.pipeAudio(pcmStream);
    } else {
      // Sonos, Teufel, AirPlay HTTP fallback: point to our HTTP stream URL
      device.startAudio(this.audioStreamUrl).catch((err) => {
        console.error(`[multiplexer] Failed to start audio on ${device.name}: ${err.message}`);
      });
    }
  }

  private stopAllDeviceAudio(): void {
    for (const [id, device] of this.devices) {
      device.stopAudio().catch(() => {});
      this.streamManager.unsubscribe(id);
    }
  }

  async start(): Promise<void> {
    console.log('[multiplexer] Starting Audio Multiplexer...');
    console.log(`[multiplexer] Receiver name: ${this.config.receiverName}`);
    console.log(`[multiplexer] HTTP port: ${this.config.httpPort}`);
    console.log(`[multiplexer] Audio stream URL: ${this.audioStreamUrl}`);

    // Start device discovery
    await this.discovery.start();

    // Start shairport-sync (AirPlay receiver)
    try {
      await this.shairport.start();
      console.log('[multiplexer] Shairport-sync started successfully');
    } catch (err: any) {
      console.warn(`[multiplexer] Shairport-sync not available: ${err.message}`);
      console.warn('[multiplexer] Running without AirPlay receiver - use audio stream endpoint for testing');
    }

    this.emitStatusChanged();
  }

  async stop(): Promise<void> {
    console.log('[multiplexer] Stopping Audio Multiplexer...');

    this.stopAllDeviceAudio();

    // Disconnect all devices
    for (const [id, device] of this.devices) {
      await device.disconnect().catch(() => {});
    }
    this.devices.clear();

    await this.shairport.stop();
    await this.discovery.stop();
    this.streamManager.cleanup();

    console.log('[multiplexer] Stopped');
  }

  // --- Public control methods ---

  async setDeviceVolume(id: string, volume: number): Promise<void> {
    const device = this.devices.get(id);
    if (!device) throw new Error(`Device not found: ${id}`);
    await device.setVolume(volume);
  }

  async setDeviceMute(id: string, muted: boolean): Promise<void> {
    const device = this.devices.get(id);
    if (!device) throw new Error(`Device not found: ${id}`);
    await device.setMute(muted);
  }

  async setDeviceEnabled(id: string, enabled: boolean): Promise<void> {
    const device = this.devices.get(id);
    if (!device) throw new Error(`Device not found: ${id}`);

    device.setEnabled(enabled);

    if (enabled && this.streaming && device.getState().connected) {
      this.startDeviceAudio(device);
    } else if (!enabled) {
      await device.stopAudio();
      this.streamManager.unsubscribe(id);
    }
  }

  async setMasterVolume(volume: number): Promise<void> {
    const promises: Promise<void>[] = [];
    for (const [, device] of this.devices) {
      promises.push(device.setVolume(volume));
    }
    await Promise.allSettled(promises);
  }

  getStatus(): MultiplexerStatus {
    const deviceList = Array.from(this.devices.values()).map((device) => ({
      ...device.config,
      ...device.getState(),
    }));

    return {
      receiverRunning: this.shairport.isRunning(),
      receiverName: this.config.receiverName,
      streaming: this.streaming,
      metadata: this.metadata,
      devices: deviceList,
      httpPort: this.config.httpPort,
    };
  }

  getStreamManager(): StreamManager {
    return this.streamManager;
  }

  private emitStatusChanged(): void {
    this.emit('statusChanged');
  }
}
