import { Message, MessageEmbed } from 'discord.js';
import request from 'request';

module.exports = {
  name: 'warState',
  description: 'Displays the war state.',
  async execute(msg: Message, args: string[]) {
    const warStateURL =
      'https://war-service-live-2.foxholeservices.com/api/worldconquest/war';

    request(warStateURL, async function (error, response, body) {
      const data = JSON.parse(body);
      let startTime = new Date(data.conquestStartTime);
      const warState = new MessageEmbed()
        .setColor('RED')
        .setFields(
          { name: 'War Number', value: `${data.warNumber}` },
          { name: 'Winner', value: `${data.winner}` },
          { name: 'Conquest Start Time', value: `${startTime}` },
          {
            name: 'Required Victory Towns',
            value: `${data.requiredVictoryTowns}`,
          }
        )
        .setFooter('Requested at')
        .setTimestamp(new Date());

      try {
        await msg.reply({ embeds: [warState] });
      } catch (err) {
        console.log(err);
      }
    });
  },
};
