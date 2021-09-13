import { Canvas, createCanvas, loadImage } from 'canvas';
import { Message, MessageAttachment, MessageEmbed } from 'discord.js';
import request from 'request';
import path from 'path';

module.exports = {
  name: 'getMap',
  descriptio: 'Generates a Hex Image of the map with icons.',
  async execute(msg: Message, args: string[]) {
    const dynamicMapDataUrl = `https://war-service-live-2.foxholeservices.com/api/worldconquest/maps/${args[0]}/dynamic/public`;
    // console.log(dynamicMapDataUrl);
    if (args[0]) {
      request(dynamicMapDataUrl, async function (error, response, body) {
        const data = JSON.parse(body);

        if (response.statusCode == 404) {
          msg.reply({
            content: 'There was nothing found for the Map/Hex Specified!',
          });
        } else if (response.statusCode == 200) {
          const canvas = createCanvas(1024, 888);
          const ctx = canvas.getContext('2d');
          const map = await loadImage(
            path.resolve(__dirname, `./Images/MapsPNG/Map${args[0]}.png`)
          );
          ctx.drawImage(map, 0, 0, canvas.width, canvas.height);

          for (var i = 0; i < data.mapItems.length; i++) {
            if (data.mapItems[i].teamId == 'WARDENS') {
              const iconWardens = await loadImage(
                path.resolve(
                  __dirname,
                  `./Images/MapIconsPNG/${data.mapItems[i].iconType}${data.mapItems[i].teamId}.png`
                )
              );
              // console.log(data.mapItems[i].iconType + '-' + `${iconWardens.src}`);
              ctx.drawImage(
                iconWardens,
                data.mapItems[i].x * 1000 * 1.005,
                data.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else if (data.mapItems[i].teamId == 'COLONIALS') {
              const iconColonials = await loadImage(
                path.resolve(
                  __dirname,
                  `./Images/MapIconsPNG/${data.mapItems[i].iconType}${data.mapItems[i].teamId}.png`
                )
              );
              // console.log(
              //   data.mapItems[i].iconType + '-' + `${iconColonials.src}`
              // );
              ctx.drawImage(
                iconColonials,
                data.mapItems[i].x * 1000 * 1.005,
                data.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            } else {
              const iconNoTeam = await loadImage(
                path.resolve(
                  __dirname,
                  `./Images/MapIconsPNG/${data.mapItems[i].iconType}.png`
                )
              );
              // console.log(data.mapItems[i].iconType + '-' + `${iconNoTeam.src}`);
              ctx.drawImage(
                iconNoTeam,
                data.mapItems[i].x * 1000 * 1.005,
                data.mapItems[i].y * 1000 * 0.85,
                24,
                24
              );
            }
          }

          await send(canvas);
        }
      });

      async function send(canvas: Canvas) {
        const attachment = new MessageAttachment(
          canvas.toBuffer(),
          'image.png'
        );

        const myEmbed = new MessageEmbed()
          .setColor('GREEN')
          .setImage('attachment://image.png')
          .setFooter('Requested at')
          .setTimestamp(new Date());

        try {
          await msg.reply({ embeds: [myEmbed], files: [attachment] });
        } catch (err) {
          console.log(err);
        }
      }
    } else {
      msg.reply({
        content:
          'Please specify the Map/Hex! You can get a list of Maps/Hex with the command !maps',
      });
    }
  },
};
