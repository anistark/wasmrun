%define __spec_install_post %{nil}
%define __os_install_post %{_dbpath}/brp-compress
%define debug_package %{nil}

Name: wasmrun
Summary: A WebAssembly Runtime
Version: @@VERSION@@
Release: @@RELEASE@@%{?dist}
License: MIT
Group: Applications/System
Source0: %{name}-%{version}.tar.gz
URL: https://github.com/anistark/wasmrun

BuildRoot: %{_tmppath}/%{name}-%{version}-%{release}-root

%description
%{summary}

A WebAssembly runtime that brings WASM to the command line with built-in UI support for running, debugging, and managing WebAssembly modules.

%prep
%setup -q

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
cp -a * %{buildroot}

%clean
rm -rf %{buildroot}

%files
%defattr(-,root,root,-)
%{_bindir}/*
