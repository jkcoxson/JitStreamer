// jkcoxson

const { exec } = require('child_process');
const { MessageAttachment } = require('discord.js');
const { Readable } = require('stream');
const fs = require('fs');

const util = require('node:util')
const execPromise = util.promisify(exec);

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
        this.client.on('interactionCreate', async (interaction) => {
            if (!interaction.isCommand()) return;
            if (interaction.command.name !== 'add-device') return;

            let deviceName = interaction.options.get('name').value;
            let deviceUDID = interaction.options.get('udid').value;
            let devicePairingFile = interaction.options.get('pairing-file').attachment.attachment;


            console.log('deviceName: ' + deviceName);
            console.log('deviceUDID: ' + deviceUDID);

            // Get the user from the database
            let devices = this.client.database.get(interaction.member.user.id);
            if (devices === undefined) {
                devices = [];
            }
            // Determine if a device by the same name or UDID already exists
            let device = devices.find(device => device.name === deviceName || device.udid === deviceUDID);
            if (device !== undefined) {
                interaction.reply('A device with the name or UDID already exists.');
                return;
            }
            // Run 'wg' to generate the keys
            let privateKey = '';
            let publicKey = '';
            let psKey = '';
            const { stdout, stderr } = await execPromise('wg genkey');
            privateKey = stdout;
            const { stdout2, stderr2 } = await execPromise('./pubkey.sh ' + privateKey);
            publicKey = stdout2;
            const { stdout3, stderr3 } = await execPromise('wg genpsk');
            psKey = stdout3;
            device = {
                name: deviceName,
                udid: deviceUDID,
                privateKey: privateKey,
                publicKey: publicKey,
                psKey: psKey
            }
            devices.push(device);
            this.client.database.set(interaction.member.user.id, devices);

            // Write the pairing file 
            let pairingFile = new Readable();
            pairingFile.push(devicePairingFile);
            pairingFile.push(null);
            fs.writeFileSync('/var/lib/lockdown/' + deviceUDID + '.conf', '');
            pairingFile.pipe(fs.createWriteStream('/var/lib/lockdown/' + deviceUDID + '.plist'));

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
                data += '\n# BEGIN_PEER ' + deviceUDID;
                data += '\n[Peer]\nPublicKey = ' + publicKey
                data += '\nAllowedIPs = ' + ip + '/32';
                data += '\nPresharedKey = ' + psKey;
                data += '\n# END_PEER ' + deviceUDID;
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
