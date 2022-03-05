// jkcoxson

const { exec } = require('child_process');
const { MessageAttachment } = require('discord.js');
const { Readable } = require('stream');

module.exports = class AddDeviceCommand {
    /**
     * @param {import('discord.js').Client} client
     */
    constructor(client) {
        this.client = client;
        this.name = 'add-device';
        this.command = {
            name: 'add-device',
            description: 'Adds a device to the database',
            options: [
                {
                    name: 'name',
                    type: 3,
                    description: 'The name of the device to add',
                    required: true
                },
                {
                    name: 'udid',
                    type: 3,
                    description: 'The UDID of the device to add',
                    required: true
                },
                {
                    name: 'pairing-file',
                    type: 11,
                    description: 'The pairing file for the device to add',
                    required: true
                }
            ]
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
            if (interaction.command.name !== 'add-device') return;

            // Get the user from the database
            let devices = this.client.database.get(interaction.member.user.id);
            if (devices === undefined) {
                devices = [];
            }
            // Determine if a device by the same name or UDID already exists
            let device = devices.find(device => device.name === interaction.command.options.name.value || device.udid === interaction.command.options.udid.value);
            if (device !== undefined) {
                interaction.reply('A device with the name or UDID already exists.');
                return;
            }
            // Run 'wg' to generate the keys
            let privateKey = '';
            let publicKey = '';
            let psKey = '';
            exec('wg genkey', (error, stdout, stderr) => {
                if (error) {
                    interaction.reply('An error occurred while generating the private key.');
                    return;
                }
                if (stderr) {
                    interaction.reply('An stderr occurred while generating the private key.');
                    return;
                }
                privateKey = stdout;
            });
            exec('wg pubkey <<<' + privateKey, (error, stdout, stderr) => {
                if (error) {
                    interaction.reply('An error occurred while generating the public key.');
                    return;
                }
                if (stderr) {
                    interaction.reply('An stderr occurred while generating the public key.');
                    return;
                }
                publicKey = stdout;
            });
            exec('wg genpsk', (error, stdout, stderr) => {
                if (error) {
                    interaction.reply('An error occurred while generating the PSK.');
                    return;
                }
                if (stderr) {
                    interaction.reply('An stderr occurred while generating the PSK.');
                    return;
                }
                psKey = stdout;
            });
            device = {
                name: interaction.command.options.getString('name'),
                udid: interaction.command.options.getString('udid'),
                privateKey: privateKey,
                publicKey: publicKey,
                psKey: psKey
            }
            devices.push(device);
            this.client.database.set(interaction.member.user.id, devices);

            // Move the pairing file to /var/lib/lockdownd
            interaction.command.options.pairingFile.value.download((error, data) => {
                if (error) {
                    interaction.reply('An error occurred while downloading the pairing file.');
                    return;
                }
                fs.writeFile('/var/lib/lockdownd/' + interaction.command.options.getString('udid') + '.plist', data, (error) => {
                    if (error) {
                        interaction.reply('An error occurred while writing the pairing file.');
                        return;
                    }
                });
            });

            // Add peer to /etc/wireguard/wg0.conf
            fs.readFile('/etc/wireguard/wg0.conf', (error, data) => {
                if (error) {
                    interaction.reply('An error occurred while reading the wireguard config file.');
                    return;
                }
                takenIps = [];
                let lines = data.toString().split('\n');
                for (let i = 0; i < lines.length; i++) {
                    if (lines[i].startsWith('AllowedIPs')) {
                        let ip = lines[i].split('=')[1].split(',')[0].trim();
                        // Get the last 2 octets of the IP
                        ip = ip.substring(ip.lastIndexOf('.') + 1);
                        takenIps.push(ip);
                    }
                }
                let ip = '';
                for (let i = 0; i < 255; i++) {
                    if (takenIps.indexOf(i.toString()) === -1) {
                        ip = i.toString();
                        break;
                    }
                }
                if (ip === '') {
                    interaction.reply('No available IPs.');
                    return;
                }

                let config = data.toString();
                data += '\n# BEGIN_PEER ' + interaction.command.options.getString('udid');
                data += '\n[Peer]\nPublicKey = ' + publicKey
                data += '\nAllowedIPs = ' + ip + '/32';
                data += '\nPresharedKey = ' + psKey;
                data += '\n# END_PEER ' + interaction.command.options.getString('udid');
                fs.writeFile('/etc/wireguard/wg0.conf', data, (error) => {
                    if (error) {
                        interaction.reply('An error occurred while writing the wireguard config file.');
                        return;
                    }
                });

                // Create the client config file
                let clientConfig = '[Interface]\nAddress = 10.7.0.' + ip + '/24\nPrivateKey = ' + privateKey
                clientConfig += '\nDNS = 8.8.8.8';
                clientConfig += '\n[Peer]\nPublicKey = ' + publicKey
                clientConfig += '\nAllowedIPs = 10.7.0.0/16';
                clientConfig += '\nPresharedKey = ' + psKey;
                clientConfig += '\nEndpoint = 149.28.211.84:51820';
                clientConfig += '\nPersistentKeepalive = 25';
                let s = new Readable()
                s.push(clientConfig);
                s.push(null);
                // Reply to the interaction with the config file
                const attatchment = new MessageAttachment(s, 'JIT.conf')
                const embed = new MessageEmbed()
                    .setTitle('JIT Client Config')
                    .setDescription('This is the client config file for the JIT device.')
                    .attachFiles(attatchment)
                    .setColor("0x00AE86")
                interaction.reply({ content: '  ', embeds: [embed] });
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
