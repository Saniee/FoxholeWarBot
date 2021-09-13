import { BaseClient, Collection } from 'discord.js';

export class Client<Ready extends boolean = boolean> extends BaseClient {
  commands: Collection<unknown, any>;
}
