"""kontract — algèbre des contrats financiers (cœur Rust).

La syntaxe Python fluide (surcharge d'opérateurs, helpers `call`, `put`, ...)
est branchée au jalon J8 par-dessus le module natif `_kontract`.
"""

try:
    from ._kontract import __version__  # module natif Rust
except ImportError:  # pas encore construit
    __version__ = "0.0.0-dev"

__all__ = ["__version__"]
