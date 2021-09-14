import { Message, MessageEmbed } from 'discord.js';
import request from 'request';

module.exports = {
  name: 'maps',
  description: 'Displays Active Maps/Tiles',
  async execute(msg: Message, args: string[]) {
    const mapsURL =
      'https://war-service-live-2.foxholeservices.com/api/worldconquest/maps';

    request(mapsURL, async function (error, response, body) {
      console.log(
        `Api request for: maps, Response Code: ${response.statusCode}`
      );

      const data = JSON.parse(body);

      const maps = new MessageEmbed()
        .setColor('AQUA')
        .setFooter('Requested at')
        .setTimestamp(new Date());
      for (var i = 0; i < data.length; i++) {
        maps.addField(`Map ${i}`, data[i]);
      }

      try {
        await msg.reply({ embeds: [maps] });
      } catch (err) {
        console.log(err);
      }
    });
  },
};
