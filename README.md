# What is this for?
This crate serves to facilitate communicate between the code running on the coprocessor and path-gui-editor by providing a communication utilities that both codebases can use.

# What utilities does this crate aim to provide?
1. A logging implementation that implements `log::Log`. This logging implementation can send logs over a network. A way to interface with said logger in a pull configuration is also provided.
2. Structures to represent robot specific information such as autonomous paths as well as way to serialise and deserialise these paths using serde.

# What does this look like in practice?


# Crate structure
