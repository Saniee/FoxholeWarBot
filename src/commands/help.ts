import { Message, MessageEmbed } from 'discord.js';

module.exports = {
  name: 'help',
  description: 'Sends Help',
  async execute(msg: Message, args: string[]) {
    const help = new MessageEmbed()
      .setColor('AQUA')
      .setDescription('Made by Saniee#0007')
      .setTitle('Commands:')
      .addFields(
        {
          name: 'help',
          value: 'This command.',
        },
        {
          name: 'getMap {Map/Hex}',
          value: 'Gets info about a Map/Hex',
        },
        {
          name: 'maps',
          value: 'Lists all active maps',
        },
        {
          name: 'warReport {Map/Hex}',
          value: 'Displays War Report for the Map/Hex',
        },
        {
          name: 'warState',
          value: 'Displays the war state.',
        }
      );

    try {
      await msg.reply({ embeds: [help] });
    } catch (err) {
      console.log(err);
    }
  },
};
