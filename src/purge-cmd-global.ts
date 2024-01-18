import { REST, Routes } from "discord.js";
import { clientId, token } from "../config.json";

const rest = new REST().setToken(token);

rest
  .put(Routes.applicationCommands(clientId), { body: [] })
  .then(() => console.log("Successfully deleted all guild commands."))
  .catch(console.error);
