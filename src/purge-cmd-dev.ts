import { REST, Routes } from "discord.js";
import { clientId, guildId, token } from "../config.json";

const rest = new REST().setToken(token);

rest
  .put(Routes.applicationGuildCommands(clientId, guildId), { body: [] })
  .then(() => console.log("Successfully deleted all guild commands."))
  .catch(console.error);
