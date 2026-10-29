"""Offline transition model only: no credentials, persistence, network or service.

The future credential manager must durably commit these states under an
interprocess lock. This model by itself is not a credential manager.
"""
from enum import Enum


class Phase(Enum):
    READY = "ready"
    REFRESHING = "refreshing"
    REAUTHORIZE = "reauthorize"


def begin(phase):
    if phase is not Phase.READY:
        raise ValueError("Cannot start refresh without a ready family")
    return Phase.REFRESHING


def complete(phase):
    if phase is not Phase.REFRESHING:
        raise ValueError("No refresh is in progress")
    return Phase.READY


def uncertain(phase):
    if phase is not Phase.REFRESHING:
        raise ValueError("No refresh is in progress")
    return Phase.REAUTHORIZE


def restart(phase):
    # The old token may already be consumed; never retry an uncertain exchange.
    return Phase.REAUTHORIZE if phase is Phase.REFRESHING else phase
