import { EventEmitter } from 'events';
import { PassThrough } from 'stream';
import { DeviceConfig, DeviceState } from '../types';

export abstract class AudioDevice extends EventEmitter {
  readonly config: DeviceConfig;
  protected _volume = 50;
  protected _muted = false;
  protected _enabled = true;
  protected _connected = false;
  protected _playing = false;

  constructor(config: DeviceConfig) {
    super();
    this.config = config;
  }

  abstract connect(): Promise<void>;
  abstract disconnect(): Promise<void>;
  abstract startAudio(streamUrl: string): Promise<void>;
  abstract stopAudio(): Promise<void>;

  async setVolume(volume: number): Promise<void> {
    this._volume = Math.max(0, Math.min(100, volume));
    this.emitStateChanged();
  }

  async setMute(muted: boolean): Promise<void> {
    this._muted = muted;
    this.emitStateChanged();
  }

  setEnabled(enabled: boolean): void {
    this._enabled = enabled;
    this.emitStateChanged();
  }

  getState(): DeviceState {
    return {
      volume: this._volume,
      muted: this._muted,
      enabled: this._enabled,
      connected: this._connected,
      playing: this._playing,
    };
  }

  protected emitStateChanged(): void {
    this.emit('stateChanged', this.getState());
  }

  get id(): string {
    return this.config.id;
  }

  get name(): string {
    return this.config.name;
  }
}
