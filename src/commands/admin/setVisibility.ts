import { CommandInteraction, SlashCommandBuilder } from "discord.js";
import { CollectionName } from "../../../config.json";
import PocketBase from "pocketbase";

module.exports = {
  data: new SlashCommandBuilder()
    .setName("set-visibility")
    .setDescription(
      "Sets the visibility of messages. This command will always be hidden."
    )
    .addBooleanOption((option) =>
      option
        .setName("visibility")
        .setDescription(
          "True shows commands to everyone. False to only the one who called the command."
        )
        .setRequired(true)
    ),
  async execute(interaction: CommandInteraction, pb: PocketBase) {
    const record = await pb
      .collection(CollectionName)
      .getFirstListItem(`guildId=${interaction.guildId}`)
      .catch((err) =>
        console.log(`No record for ${interaction.guild?.name} found!`)
      );

    var visibility = interaction.options.get("visibility")?.value;

    await interaction.deferReply({ ephemeral: true });

    const data = {
      showCommandOutput: visibility,
    };
    if (record) {
      await pb
        .collection(CollectionName)
        .update(record.id, data)
        .then(() =>
          console.log(`Updated Record for ${interaction.guild?.name}!`)
        )
        .catch((err) => console.log(err));
      await interaction.editReply("Saved!");
    } else {
      await interaction.editReply({
        content:
          "Server setting missing, please run the command `/set-server` to fix this issue!",
      });
    }
  },
};
