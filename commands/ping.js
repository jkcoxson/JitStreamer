// jkcoxson

module.exports = class Ping {
    /**
     * @param {import('discord.js').Client} client
     */
    constructor(client) {
        this.client = client;
        this.name = 'ping';
        this.command = {
            name: 'ping',
            description: 'Just another ping command',
        }
    }

    /**
     * Initializes the command
     */
    init() {
        // Create a new command
        console.log('Registering command: ' + this.name);
        this.postCommand();

        // Add a callback for the command
        this.client.on('interactionCreate', (interaction) => {
            if (!interaction.isCommand()) return;
            if (interaction.command.name !== 'ping') return;

            // Ping the author of the interaction 10 times
            interaction.reply('<@' + interaction.member.user.id + '>').then(() => {
                for (let i = 0; i < 2; i++) {
                    interaction.followUp('<@' + interaction.member.user.id + '>');
                }
                interaction.followUp('Happy?');
            });

        });
    }

    postCommand() {
        this.client.guilds.cache.forEach(guild => {
            this.client.application.commands.create(this.command, guild.id);
        });
    }

    /**@type {import('discord.js').Client} */
    client

    /**@type {String} */
    name

    command

}
