import { Message, MessageEmbed } from 'discord.js';
import request from 'request';
import serverConfig from '../mongodb/serverConfig';

module.exports = {
  name: 'maps',
  description: 'Displays Active Maps/Tiles',
  async execute(msg: Message, args: string[]) {
    serverConfig.findOne(
      { guildID: msg.guildId },
      async (err: any, mongoDB: any) => {
        if (mongoDB) {
          const mapsURL = `${mongoDB.shard}/api/worldconquest/maps`;

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
              msg.author
                .send(
                  'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ'
                )
                .catch((err) => {
                  console.log(err);
                });
            }
          });
        } else if (!mongoDB) {
          try {
            msg.reply({
              content:
                'Shard setting missing, please run the command `War!setShard {shard1 | shard2}` to fix this issue!',
            });
          } catch (err) {
            console.log(err);
            msg.author
              .send(
                'Please enable all needed permisions. Or wait for an issue to be fixed. Support server: https://discord.gg/9wzppSgXdQ'
              )
              .catch((err) => {
                console.log(err);
              });
          }
        }
      }
    );
  },
};
