{
  foo = builtins.derivation {
    name = "foo";
    system = builtins.currentSystem;
    builder = "/bin/sh";
    meow = ./meowmeow;
    directory = builtins.toJSON (builtins.readDir ./dir);
    var = builtins.getEnv "MEOWMEOW";
  };
}
