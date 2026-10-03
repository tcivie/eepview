# SPDX-FileCopyrightText: 2026 The eepview contributors
# SPDX-License-Identifier: MIT

def refuse_erlang_without_pygments(*_args, **_kwargs):
    message = "pygments is not installed: lizard cannot read Erlang here"
    raise RuntimeError(message)


lex = refuse_erlang_without_pygments
