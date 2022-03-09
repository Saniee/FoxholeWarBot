import axios from 'axios';
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

          const response = axios.get(
            `${mongoDB.shard}/api/worldconquest/maps`,
            {
              validateStatus: function (status) {
                return status < 500;
              },
            }
          );

          console.log(
            `Api request for: maps, Response Code: ${(await response).status}`
          );

          const data: any = (await response).data;
          let mapHexes = ""

          for (var i = 0; i < data.length; i++) {
            mapHexes = mapHexes + ` ${data[i]}\n`
          }
        
          const maps = new MessageEmbed()
            .setColor('AQUA')
            .setFooter('Requested at')
            .setTimestamp(new Date())
            .setTitle("All Hexes:")
            .setDescription(`${mapHexes}`)

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
