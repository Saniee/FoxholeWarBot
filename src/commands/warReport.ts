import { Message, MessageEmbed } from 'discord.js';
import request from 'request';

module.exports = {
  name: 'warReport',
  description: 'Displays War Report for the Map/Hex',
  async execute(msg: Message, args: string[]) {
    const warReportURL = `https://war-service-live-2.foxholeservices.com/api/worldconquest/warReport/${args[0]}`;

    if (args[0]) {
      request(warReportURL, async function (error, response, body) {
        const data = JSON.parse(body);
        switch (response.statusCode) {
          case 404:
            msg.reply({ content: 'War Report for that map wasnt found!' });
            break;
          case 200:
            const warReport = new MessageEmbed()
              .setFields(
                {
                  name: 'Total Enlistments',
                  value: `${data.totalEnlistments}`,
                },
                {
                  name: 'Colonial Casualties',
                  value: `${data.colonialCasualties}`,
                },
                {
                  name: 'Warden Casualties',
                  value: `${data.wardenCasualties}`,
                },
                {
                  name: 'Day Of War',
                  value: `${data.dayOfWar}`,
                }
              )
              .setFooter('Requested at')
              .setTimestamp(new Date());
            try {
              await msg.reply({ embeds: [warReport] });
            } catch (err) {
              console.log(err);
            }
            break;
        }
      });
    } else {
      msg.reply({ content: 'Please specify the Map/Hex for the War Report!' });
    }
  },
};
