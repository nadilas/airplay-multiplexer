import { EventEmitter } from 'events';

export type DeviceType = 'sonos' | 'teufel' | 'airplay';

export interface DeviceConfig {
  id: string;
  name: string;
  host: string;
  port: number;
  type: DeviceType;
  location?: string;
  model?: string;
}

export interface DeviceState {
  volume: number;
  muted: boolean;
  enabled: boolean;
  connected: boolean;
  playing: boolean;
}

export interface AudioFormat {
  sampleRate: number;
  bitDepth: number;
  channels: number;
}

export interface TrackMetadata {
  artist?: string;
  title?: string;
  album?: string;
}

export interface MultiplexerStatus {
  receiverRunning: boolean;
  receiverName: string;
  streaming: boolean;
  metadata: TrackMetadata;
  devices: Array<DeviceConfig & DeviceState>;
  httpPort: number;
}

export interface DeviceEvents {
  connected: () => void;
  disconnected: () => void;
  error: (err: Error) => void;
  stateChanged: (state: DeviceState) => void;
}

export declare interface TypedEmitter {
  on<K extends keyof DeviceEvents>(event: K, listener: DeviceEvents[K]): this;
  emit<K extends keyof DeviceEvents>(event: K, ...args: Parameters<DeviceEvents[K]>): boolean;
}
