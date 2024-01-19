#!/bin/bash
## Script for building/installing FreeSWITCH from source.
## URL: https://gist.github.com/mariogasparoni/2f21fd03afa27beae656b70ff7826f24
## Freely distributed under the MIT license
##
##
set -xe
FREESWITCH_SOURCE=https://github.com/signalwire/freeswitch.git
FREESWITCH_RELEASE=v1.10.10
PREFIX=/usr/share/freeswitch

#Clean old prefix and build
sudo rm -rf $PREFIX
rm -rf ~/build-$FREESWITCH_RELEASE

#install dependencies
sudo apt-get update && sudo apt-get install -y git-core build-essential python-is-python3 python3-dev autoconf automake libtool libncurses5 libncurses5-dev make libjpeg-dev pkg-config zlib1g-dev sqlite3 libsqlite3-dev libpcre3-dev libspeexdsp-dev libspeex-dev libedit-dev libldns-dev liblua5.1-0-dev libcurl4-gnutls-dev libapr1-dev yasm libsndfile-dev libopus-dev libtiff-dev libavformat-dev libswscale-dev libswresample-dev libpq-dev zip libwebsockets-dev


#clone source and prepares it
mkdir -p ~/build-$FREESWITCH_RELEASE
cd ~/build-$FREESWITCH_RELEASE

PVERSION=( ${FREESWITCH_RELEASE//./ } )
MIN_VERSION=${PVERSION[1]}
PATCH_VERSION=${PVERSION[2]}

if [[ $FREESWITCH_RELEASE = "master" ]] || [[ $MIN_VERSION -ge 10  &&  $PATCH_VERSION -ge 3 ]]
then
    echo "VERSION => 1.10.3 - need to build spandsp and sofia-sip separatedly"

    #build and install libspandev
    git clone https://github.com/freeswitch/spandsp.git
    cd spandsp
    #https://github.com/freeswitch/spandsp/pull/59/files - fix
    git reset --hard 0d2e6ac65e0e8f53d652665a743015a88bf048d4
    ./bootstrap.sh
    ./configure --prefix=$PREFIX
    make
    sudo make install
    cd ..
    
    #build and install mod_sofia
    git clone https://github.com/freeswitch/sofia-sip.git
    cd sofia-sip
    ./bootstrap.sh
    ./configure --prefix=$PREFIX
    make
    sudo make install
    cd ..
fi


git clone https://github.com/bhavinmoradiya447/drachtio-freeswitch-modules.git
cd drachtio-freeswitch-modules
git checkout with-zmq
cd ..

#avoid git access's denied error
touch .config && sudo chown $USER:$USER .config
if [ ! -d freeswitch ]
then
    git clone $FREESWITCH_SOURCE freeswitch
    cd freeswitch
else
    cd freeswitch
    git fetch origin
fi
git reset --hard $FREESWITCH_RELEASE && git clean -d -x -f


#cp /usr/src/freeswitch/src/mod/endpoints/mod_sofia/sofia_glue.c src/mod/endpoints/mod_sofia/sofia_glue.c
#cp /usr/src/freeswitch/src/switch_rtp.c src/switch_rtp.c

cp ../drachtio-freeswitch-modules/modules/mod_audio_fork src/mod/applications/ -r 

#remove mod_signalwire from building
sed -i "s/applications\/mod_signalwire/#applications\/mod_signalwire/g" build/modules.conf.in
sed -i "s/databases\/mod_pgsql/#databases\/mod_pgsql/g" build/modules.conf.in
echo "applications/mod_audio_fork" >> build/modules.conf.in

# Update configure.ac and add path into AC_CONFIG_FILES([Makefile

./bootstrap.sh

#configure , build and install
env PKG_CONFIG_PATH=$PREFIX/lib/pkgconfig ./configure --prefix=$PREFIX --disable-libvpx
env C_INCLUDE_PATH=$PREFIX/include make
sudo make install config-vanilla


cp /etc/freeswitch /usr/share/freeswitch/etc/ -r 
