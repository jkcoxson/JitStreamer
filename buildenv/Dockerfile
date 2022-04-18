ARG IMAGE=arm64v8/alpine:3.15

FROM $IMAGE

RUN apk add git pkgconfig autoconf automake libtool make alpine-sdk python3 openssl-libs-static

RUN mkdir /buildenv

# Build the dependencies

WORKDIR /buildenv

RUN git clone https://github.com/libimobiledevice/libplist.git; 
WORKDIR /buildenv/libplist
RUN ./autogen.sh --enable-static --disable-shared --without-cython; make; make install;

WORKDIR /buildenv

RUN git clone https://github.com/libimobiledevice/libimobiledevice-glue.git; 
WORKDIR /buildenv/libplist
RUN ./autogen.sh --enable-static --disable-shared --without-cython; make; make install;

WORKDIR /buildenv

RUN git clone https://github.com/libimobiledevice/libusbmuxd.git; 
WORKDIR /buildenv/libplist
RUN ./autogen.sh --enable-static --disable-shared --without-cython; make; make install;

WORKDIR /buildenv

RUN git clone https://github.com/libimobiledevice/libimobiledevice.git; 
WORKDIR /buildenv/libplist
RUN ./autogen.sh --enable-static --disable-shared --without-cython; make; make install;

WORKDIR /buildenv

