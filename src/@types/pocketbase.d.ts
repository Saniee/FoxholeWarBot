import { RecordModel } from 'pocketbase';

declare module 'pocketbase' {
    export interface RecordModel {
        guildId: number;
        shard: string;
        shardName: string;
        showCommandOutput: boolean;
    }
}
